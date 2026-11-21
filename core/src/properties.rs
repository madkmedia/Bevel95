//! The two property sheets. Taskbar Properties has Windows' own pages (Taskbar Options and
//! Start Menu Programs; in Windows 2000 General and Advanced), drawn as Windows drew them.
//! Bevel95 Settings is the same kind of window for everything that is Bevel95's own (the look,
//! the size, starting with Windows, its clock; on a second page the Windows key and grouping
//! the task buttons; on a third its desktop; on a fourth what the Programs menu shows), so that Windows' window holds only what Windows' had. `Sheet`
//! says which of the two a window is.
//!
//! Windows 95 (with Internet Explorer 4's taskbar), Windows 98 and Windows 2000, run in the
//! v86 emulator and read pixel for pixel, show windows of the same size and frame; 95 and 98
//! are identical, preview picture included. Every position here comes from there
//! (docs/theme-spec.md, "Taskbar Properties"); the preview is put together from five pictures
//! cut from it, as Windows put it together from its own. Since 2026-11-18 every picture is the
//! Reference PCs' own (`art/src/assets.rs`): 98's and 2000's found identical on their PCs, and the
//! 95 look has plain Windows 95's Start menus and taskbar (its PC's Taskbar Properties; the window
//! and the frame's places are the same, `vm/own-pictures`). Bevel95 Settings' page is ours, built
//! from the same parts. This module only lays out and draws; the platform layer owns the
//! windows and carries out the choices.

use crate::config::{MENU_SCALES, StartOptions, TaskbarOptions, WindowsKey};
use crate::dialog::{self, OPTION_OFF, OPTION_ON, TitleButtons};
use crate::fb::{Framebuffer, Rect};
use crate::kit::{self, Effect, Key, Mods, Press};
use crate::paint::CHECK_ART;
use crate::programs::{self, Row, RowKind, Shown, Tick};
use crate::scale::{Scale, Way};
use crate::scroll::ScrollBar;
use crate::text::{Font, Fonts, split_access_key};
use crate::theme::{Colors, Depth, Era, Theme, rgb};
use crate::tree::{self, TreeRow, TreeStyle, Zone};

/// Everything the two windows set (each puts only its own part into effect: `Sheet::merge`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Values {
    pub options: TaskbarOptions,
    pub start: StartOptions,
    pub era: Era,
    /// True Color or 16 Colors (Bevel95 Settings, General page).
    pub colors: Depth,
    /// The size, with the way a size between whole numbers is drawn.
    pub scale: Scale,
    pub start_with_windows: bool,
    /// Seconds Bevel95's clock runs ahead of the PC's (Date/Time Properties sets it; "Use the
    /// PC's Time" puts it back to 0).
    pub clock_offset: i64,
    /// Bevel95's own desktop is shown over Windows' (the desktop's own menu switches it off).
    pub desktop: bool,
    /// What the Programs menu shows (Bevel95 Settings, Programs page). The programs ticked by
    /// hand are not a value here: they are the window's list (`ProgramList`).
    pub programs: Shown,
    /// What a lone press of the Windows key opens (Bevel95 Settings, Taskbar page).
    pub windows_key: WindowsKey,
    /// A program's windows share one task button (Bevel95 Settings, Taskbar page).
    pub group: bool,
    /// Quick Search lists every entry of the Programs menu, not only its programs (Bevel95
    /// Settings, Taskbar page; `quick_search = everything`).
    pub quick_search_all: bool,
    /// The battery's percentage stands beside its icon (Bevel95 Settings, Taskbar page).
    pub battery_percent: bool,
    /// Quick Launch shows (Bevel95 Settings, Taskbar page; the bar's Toolbars menu): what holds
    /// now, the look's default while settings.txt does not say.
    pub quick_launch: bool,
}

/// The check boxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    AlwaysOnTop,
    AutoHide,
    SmallIcons,
    ShowClock,
    /// Windows 2000: menus that hide what you rarely use. Not done in Bevel95: grayed out.
    PersonalizedMenus,
    /// Bevel95 Settings: start the bar when you sign in.
    StartWithWindows,
    /// Bevel95 Settings, Desktop page: show Bevel95's own desktop.
    Desktop,
    /// Bevel95 Settings, Taskbar page: a program's windows share one button.
    Group,
    /// Bevel95 Settings, Taskbar page: Quick Search lists documents and web pages too.
    QuickSearchAll,
    /// Bevel95 Settings, Taskbar page: the battery's percentage beside its icon.
    BatteryPercent,
    /// Bevel95 Settings, Taskbar page: the Quick Launch band.
    QuickLaunch,
}

/// The entries of Windows 2000's Start Menu Settings list (Advanced page).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    AdminTools,
    Favorites,
    LogOff,
    ExpandControlPanel,
    ExpandDocuments,
    ExpandNetwork,
    ExpandPrinters,
    ScrollPrograms,
}

pub const SETTINGS: [Setting; 8] = [
    Setting::AdminTools,
    Setting::Favorites,
    Setting::LogOff,
    Setting::ExpandControlPanel,
    Setting::ExpandDocuments,
    Setting::ExpandNetwork,
    Setting::ExpandPrinters,
    Setting::ScrollPrograms,
];

impl Setting {
    pub fn label(self) -> &'static str {
        match self {
            Setting::AdminTools => "Display Administrative Tools",
            Setting::Favorites => "Display Favorites",
            Setting::LogOff => "Display Logoff",
            Setting::ExpandControlPanel => "Expand Control Panel",
            Setting::ExpandDocuments => "Expand My Documents",
            Setting::ExpandNetwork => "Expand Network and Dial-up Connections",
            Setting::ExpandPrinters => "Expand Printers",
            Setting::ScrollPrograms => "Scroll the Programs menu",
        }
    }

    /// Whether Bevel95 carries it out; the others are shown grayed out.
    pub fn supported(self) -> bool {
        matches!(self, Setting::Favorites | Setting::LogOff)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// "General" in Windows 2000.
    TaskbarOptions,
    /// "Advanced" in Windows 2000.
    StartMenuPrograms,
    /// Bevel95 Settings' first page, "General": the look, the size, starting with Windows, the
    /// clock.
    Bevel95,
    /// Bevel95 Settings' second page, "Taskbar": what the Windows key opens, whether a
    /// program's windows share a button.
    OwnTaskbar,
    /// Bevel95 Settings' third page: Bevel95's own desktop.
    Desktop,
    /// Bevel95 Settings' fourth page: what the Programs menu shows.
    Programs,
}

/// Which of the two windows: Windows' own Taskbar Properties, or Bevel95 Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    Taskbar,
    Bevel95,
}

impl Sheet {
    /// The window's pages, in tab order.
    pub fn pages(self) -> &'static [Page] {
        match self {
            Sheet::Taskbar => &[Page::TaskbarOptions, Page::StartMenuPrograms],
            Sheet::Bevel95 => &[Page::Bevel95, Page::OwnTaskbar, Page::Desktop, Page::Programs],
        }
    }

    /// What this window's OK and Apply put into effect: its own settings as it shows them
    /// (`shown`), and the other window's as they are in effect now (`in_effect`). Both windows
    /// can be open at once, so neither may put back what the other one changed meanwhile.
    pub fn merge(self, shown: Values, in_effect: Values) -> Values {
        match self {
            Sheet::Taskbar => Values { options: shown.options, start: shown.start, ..in_effect },
            Sheet::Bevel95 => Values { options: in_effect.options, start: in_effect.start, ..shown },
        }
    }
}

/// The parts of the window that the mouse and the keyboard act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    TitleHelp,
    TitleClose,
    /// The tab of page `i`. The tabs are one stop for the keyboard (the tab control).
    Tab(usize),
    Check(Check),
    /// Bevel95 Settings: the option buttons of the look.
    Look(Era),
    /// Bevel95 Settings: the option buttons of the colors.
    Colors(Depth),
    /// Bevel95 Settings, General page: the drop-down list of the sizes (every size the menus
    /// offer, `MENU_SCALES`) and the one of how a size between whole numbers is drawn
    /// (`Way::ALL`; usable only at such a size); one stop each for the keyboard.
    SizeList,
    WayList,
    /// An entry of the open drop-down list (its index in the list's choices).
    DropRow(usize),
    /// Bevel95 Settings: the option buttons of what the Windows key opens.
    WinKey(WindowsKey),
    /// Start Menu Programs page (Advanced in Windows 2000).
    Add,
    Remove,
    Advanced,
    /// Windows 2000 only.
    ReSort,
    Clear,
    /// Windows 2000's Start Menu Settings list (one stop for the keyboard), a row of it (index
    /// into `SETTINGS`), and its scroll bar's parts.
    List,
    ListRow(usize),
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollThumb,
    /// Bevel95 Settings, Programs page: the option buttons of what the menu shows, the list of
    /// programs (one stop for the keyboard) and a row of it (index into `ProgramList::rows`).
    /// The list scrolls with the scroll bar parts above: a page has one list at most.
    Shown(Shown),
    Programs,
    ProgramRow(usize),
    /// Bevel95 Settings: puts Bevel95's clock back to the PC's time.
    UsePcTime,
    Ok,
    Cancel,
    Apply,
}

impl Part {
    pub fn is_push_button(self) -> bool {
        matches!(
            self,
            Part::Add
                | Part::Remove
                | Part::Advanced
                | Part::ReSort
                | Part::Clear
                | Part::UsePcTime
                | Part::Ok
                | Part::Cancel
                | Part::Apply
        )
    }
}

/// The two designs: Internet Explorer 4's (Windows 95 with it, and 98) and Windows 2000's.
fn is_2000(era: Era) -> bool {
    era == Era::Win2000
}

/// A window's title.
pub fn title(sheet: Sheet, era: Era) -> &'static str {
    match (sheet, is_2000(era)) {
        (Sheet::Taskbar, false) => "Taskbar Properties",
        (Sheet::Taskbar, true) => "Taskbar and Start Menu Properties",
        (Sheet::Bevel95, _) => "Bevel95 Settings",
    }
}

/// A page's tab label.
pub fn page_title(era: Era, page: Page) -> &'static str {
    match (page, is_2000(era)) {
        (Page::TaskbarOptions, false) => "Taskbar Options",
        (Page::TaskbarOptions, true) => "General",
        (Page::StartMenuPrograms, false) => "Start Menu Programs",
        (Page::StartMenuPrograms, true) => "Advanced",
        (Page::Bevel95, _) => "General",
        (Page::OwnTaskbar, _) => "Taskbar",
        (Page::Desktop, _) => "Desktop",
        (Page::Programs, _) => "Programs",
    }
}

fn check_label(check: Check) -> &'static str {
    match check {
        Check::AlwaysOnTop => "Always on &top",
        Check::AutoHide => "A&uto hide",
        Check::SmallIcons => "Show &small icons in Start menu",
        Check::ShowClock => "Show &clock",
        Check::PersonalizedMenus => "Use &Personalized Menus",
        Check::StartWithWindows => "&Start Bevel95 when Windows starts",
        Check::Desktop => "&Show the classic desktop",
        // Windows XP's own wording.
        Check::Group => "&Group similar taskbar buttons",
        // Ours (PROVISIONAL), under the group's title "Quick Search".
        Check::QuickSearchAll => "Also list &documents, web pages and uninstallers",
        // Ours (PROVISIONAL), under the group's title "Battery".
        Check::BatteryPercent => "Show the &battery's percentage beside its icon",
        // Ours (PROVISIONAL), in the Buttons group.
        Check::QuickLaunch => "Show Quick &Launch",
    }
}

fn look_label(era: Era) -> &'static str {
    match era {
        Era::Win95 => "Windows &95",
        Era::Win98 => "Windows 9&8",
        Era::Win2000 => "Windows 2&000",
    }
}

/// The Windows key group's two choices (ours): Quick Search, Bevel95's own tool, has the same
/// name in every look (until 2026-11-16 "Find", "Search" in the 2000 look).
fn windows_key_label(key: WindowsKey) -> &'static str {
    match key {
        WindowsKey::Find => "Opens &Quick Search",
        WindowsKey::Start => "Opens the Start &menu",
    }
}

/// The Colors group's three choices, in the words of Windows' own Display Properties (256's
/// access key the 6: the 2 is the Size group's 2×).
fn colors_label(depth: Depth) -> &'static str {
    match depth {
        Depth::Sixteen => "16 &Colors",
        Depth::TwoFiftySix => "25&6 Colors",
        Depth::TrueColor => "&True Color",
    }
}

/// A size as its drop-down list writes it: "2×", "1.75×".
fn size_text(scale: Scale) -> String {
    format!("{scale}\u{d7}")
}

/// The labels before the two drop-down lists (ours, PROVISIONAL).
const SIZE_LIST_LABEL: &str = "Si&ze:";
const WAY_LIST_LABEL: &str = "&Drawn as:";

/// The line under them: what the size does, and at a size between whole numbers that it is not
/// pixel-exact (ours, PROVISIONAL).
const SIZE_WHOLE_TEXT: &str = "Each pixel of the classic look becomes this many.";
const SIZE_QUARTER_TEXT: &str = "A size between whole numbers is not pixel-exact.";

/// The choices of drop-down list `list` (`Part::SizeList` or `Part::WayList`).
fn list_choices(list: Part) -> Vec<String> {
    match list {
        Part::SizeList => MENU_SCALES.iter().map(|&s| size_text(s)).collect(),
        Part::WayList => Way::ALL.iter().map(|w| w.title().to_string()).collect(),
        _ => Vec::new(),
    }
}

/// The Programs page's three choices (ours; the last one's colon leads on to the list).
fn shown_label(shown: Shown) -> &'static str {
    match shown {
        Shown::Everything => "&Everything on this PC",
        Shown::Classic => "Only Bevel95's own &classic programs",
        Shown::Ticked => "Only the programs &ticked here:",
    }
}

fn button_label(era: Era, part: Part) -> &'static str {
    match (part, is_2000(era)) {
        (Part::Add, false) => "&Add...",
        (Part::Add, true) => "A&dd...",
        (Part::Remove, _) => "&Remove...",
        (Part::Advanced, false) => "A&dvanced...",
        (Part::Advanced, true) => "Ad&vanced...",
        (Part::ReSort, _) => "Re-&sort",
        (Part::Clear, _) => "&Clear",
        (Part::UsePcTime, _) => "&Use the PC's Time",
        (Part::Ok, _) => "OK",
        (Part::Cancel, _) => "Cancel",
        (Part::Apply, _) => "&Apply",
        _ => "",
    }
}

/// The label of `part` (with its access key marked by "&"), if it has one.
fn label_of(era: Era, part: Part) -> Option<String> {
    match part {
        Part::Check(c) => Some(check_label(c).to_string()),
        Part::Look(e) => Some(look_label(e).to_string()),
        Part::Colors(d) => Some(colors_label(d).to_string()),
        Part::WinKey(k) => Some(windows_key_label(k).to_string()),
        Part::SizeList => Some(SIZE_LIST_LABEL.to_string()),
        Part::WayList => Some(WAY_LIST_LABEL.to_string()),
        Part::Shown(s) => Some(shown_label(s).to_string()),
        Part::List => Some(LIST_LABEL.to_string()),
        p if p.is_push_button() => Some(button_label(era, p).to_string()),
        _ => None,
    }
}

const LIST_LABEL: &str = "Start Menu Se&ttings";

/// Bevel95 Settings' list of programs (Programs page): every program found on this PC as a
/// tree, the folders opening and closing as in Windows' own trees (`tree`), each row with a
/// box. What the boxes show follows the choice above the list (`programs::tick`). A box
/// flipped while another choice is made starts from what that choice shows and makes the
/// choice "Only the programs ticked here".
#[derive(Clone, Debug, Default)]
pub struct ProgramList {
    /// The rows (`programs::rows`), those in closed folders included. The window's host reads
    /// them when the page is first shown (finding every program and its picture can take a
    /// moment), not when the window opens.
    pub rows: Vec<Row>,
    pub loaded: bool,
    /// The selected row (its number in `rows`; always one that shows).
    pub sel: usize,
    /// How many of the rows that show are scrolled out above the list.
    pub top: usize,
    /// A box was flipped since the rows were read or last put into effect.
    pub edited: bool,
}

/// What one of the two windows shows right now.
#[derive(Clone, Debug)]
pub struct Properties {
    /// Which window it is.
    pub sheet: Sheet,
    /// The look the window is drawn in (the one in use, not the one chosen in Bevel95 Settings).
    pub era: Era,
    /// The page shown (index into the window's pages, `Sheet::pages`).
    pub page: usize,
    /// What the controls show (not in effect until OK or Apply).
    pub values: Values,
    /// Something was changed since the window opened or Apply was used: Apply is enabled.
    pub changed: bool,
    /// The part with the keyboard focus.
    pub focus: Part,
    /// The control held down with the mouse, drawn pushed in while the pointer is on it.
    pub pressed: Option<Part>,
    /// Underline access keys and draw focus rectangles: Windows 95 and 98 always do; Windows
    /// 2000 once the keyboard has been used.
    pub show_keys: bool,
    /// The Documents menu is empty, so Clear has nothing to do (grayed out).
    pub documents_empty: bool,
    /// Windows 2000's Start Menu Settings list: the selected row and the first row shown.
    pub list_sel: usize,
    pub list_top: usize,
    /// Bevel95 Settings' list of programs.
    pub programs: ProgramList,
    /// Where the window's top-left corner is on the screen, in 1× pixels (Windows lines the
    /// dots of its focus rectangles up with the screen).
    pub screen: (i32, i32),
    /// The open drop-down list (`Part::SizeList` or `Part::WayList`) and its highlighted entry.
    pub drop: Option<(Part, usize)>,
}

impl Properties {
    pub fn new(sheet: Sheet, era: Era, values: Values) -> Properties {
        Properties {
            sheet,
            era,
            page: 0,
            values,
            changed: false,
            focus: first_control(era, sheet.pages()[0], &values),
            pressed: None,
            show_keys: era.always_shows_keys(),
            documents_empty: false,
            list_sel: 0,
            list_top: 0,
            programs: ProgramList::default(),
            screen: (0, 0),
            drop: None,
        }
    }

    /// The entry of drop-down list `list` that is chosen now, if the list has it (a size set in
    /// settings.txt that the list does not offer has none).
    pub fn list_chosen(&self, list: Part) -> Option<usize> {
        match list {
            Part::SizeList => MENU_SCALES.iter().position(|s| s.quarters() == self.values.scale.quarters()),
            Part::WayList => Way::ALL.iter().position(|&w| w == self.values.scale.way),
            _ => None,
        }
    }

    /// The entry of drop-down list `list` nearest to what is chosen now: the chosen one, or for
    /// a size the list does not offer (4.25 to 8 in settings.txt) the nearest size it offers.
    fn list_near(&self, list: Part) -> usize {
        self.list_chosen(list).unwrap_or_else(|| match list {
            Part::SizeList => {
                let q = self.values.scale.quarters();
                (0..MENU_SCALES.len()).min_by_key(|&i| (MENU_SCALES[i].quarters() - q).abs()).unwrap_or(0)
            }
            _ => 0,
        })
    }

    /// What drop-down list `list` shows in its field.
    pub fn list_text(&self, list: Part) -> String {
        match list {
            Part::SizeList => size_text(self.values.scale),
            Part::WayList => self.values.scale.way.title().to_string(),
            _ => String::new(),
        }
    }

    /// Entry `i` of drop-down list `list` is chosen. False if it does not have one.
    pub fn choose_entry(&mut self, list: Part, i: usize) -> bool {
        let v = &mut self.values;
        match list {
            Part::SizeList => match MENU_SCALES.get(i) {
                Some(s) => v.scale = s.with_way(v.scale.way),
                None => return false,
            },
            Part::WayList => match Way::ALL.get(i) {
                Some(&way) => v.scale = v.scale.with_way(way),
                None => return false,
            },
            _ => return false,
        }
        self.changed = true;
        true
    }

    /// Opens drop-down list `list` on its chosen entry, or closes the open one.
    pub fn toggle_list(&mut self, list: Part) {
        self.drop = match self.drop {
            Some(_) => None,
            None if self.enabled(list) => Some((list, self.list_near(list))),
            None => None,
        };
    }

    /// The Up and Down keys (Left and Right too while it is closed) in drop-down list `list`:
    /// the open list's highlight moves; a closed list's choice moves at once, as Windows' did.
    /// A size the list does not offer goes to the nearest one it offers.
    pub fn step_list(&mut self, list: Part, forward: bool) {
        let n = list_choices(list).len();
        if n == 0 {
            return;
        }
        let step = |at: usize| if forward { (at + 1).min(n - 1) } else { at.saturating_sub(1) };
        match self.drop {
            Some((open, hover)) if open == list => self.drop = Some((open, step(hover))),
            _ => {
                let next = self.list_chosen(list).map_or_else(|| self.list_near(list), step);
                self.choose_entry(list, next);
            }
        }
    }

    /// The host has read the programs: the list shows them from its first row, the folders
    /// closed but for those that show what the choice made is about (`programs::reveal`).
    pub fn set_programs(&mut self, mut rows: Vec<Row>) {
        programs::reveal(&mut rows, self.values.programs);
        self.programs = ProgramList { rows, loaded: true, ..ProgramList::default() };
    }

    /// The host has read the programs again (another look has other pictures for Bevel95's
    /// own): the programs that are still there keep the ticks they have in the window, the
    /// folders stay open or closed, and the list stays where it was.
    pub fn refresh_programs(&mut self, mut rows: Vec<Row>) {
        let list = &mut self.programs;
        let selected = list.rows.get(list.sel).map(|row| (row.kind, row.place.clone()));
        for row in rows.iter_mut().filter(|row| row.kind != RowKind::Root) {
            let same = |old: &&Row| old.kind == row.kind && old.place.eq_ignore_ascii_case(&row.place);
            if let Some(old) = list.rows.iter().find(same) {
                (row.ticked, row.open) = (old.ticked, old.open);
            }
        }
        if let Some(root) = list.rows.first() {
            rows[0].open = root.open;
        }
        // The selection stays on its row if that is still there and still shows.
        let now = selected.and_then(|(kind, place)| rows.iter().position(|row| row.kind == kind && row.place == place));
        list.rows = rows;
        list.loaded = true;
        let shows = programs::visible(&list.rows);
        list.sel = now.filter(|at| shows.contains(at)).unwrap_or(0);
        list.top = list.top.min(shows.len().saturating_sub(PROGRAM_PAGE));
    }

    /// "Remove Shortcuts/Folders" took the programs at `places` out of the menu, which shows
    /// only the ticked ones from now on: the window shows that choice, and its list, if it has
    /// been read, those programs without their ticks (so its OK or Apply does not put them back).
    pub fn programs_removed(&mut self, places: &[String]) {
        if self.programs.loaded {
            programs::hide(&mut self.programs.rows, self.values.programs, places);
        }
        self.values.programs = Shown::Ticked;
    }

    /// Add...'s Create Shortcut wizard made the programs at `places`, and the Programs menu,
    /// which shows `menu_shows` now, shows them (`programs::unhide`; the platform layer has
    /// read the list again, so their rows are in it). If the menu shows only the ticked ones
    /// (as the classic choice becomes), the window shows that choice and the new programs
    /// ticked, so its OK or Apply does not take them out again. With everything, nothing was
    /// written and the window stays as it is.
    pub fn programs_added(&mut self, places: &[String], menu_shows: Shown) {
        if menu_shows != Shown::Ticked {
            return;
        }
        if self.programs.loaded {
            programs::unhide(&mut self.programs.rows, self.values.programs, places);
        }
        self.values.programs = Shown::Ticked;
    }

    /// What the box of row `i` of the list of programs shows now.
    pub fn row_tick(&self, i: usize) -> Tick {
        programs::tick(&self.programs.rows, i, self.values.programs)
    }

    /// The rows of the list of programs that show (their numbers in `ProgramList::rows`).
    pub fn program_rows(&self) -> Vec<usize> {
        programs::visible(&self.programs.rows)
    }

    /// The list of programs as a tree with a selection, for what keys and mouse do with it
    /// (`programs::Cursor`, shared with the Remove Shortcuts/Folders window).
    fn cursor(&mut self) -> programs::Cursor<'_> {
        let list = &mut self.programs;
        programs::Cursor { rows: &mut list.rows, sel: &mut list.sel, top: &mut list.top, page: PROGRAM_PAGE }
    }

    /// Opens or closes a folder of the list of programs. The selection, if it was inside a
    /// folder that closes, goes to that folder, as in Windows' trees.
    pub fn open_program(&mut self, i: usize, open: bool) {
        self.cursor().open(i, open);
    }

    /// Scrolls the list of programs so that row `i` shows (it must be in open folders).
    fn show_program(&mut self, i: usize) {
        self.cursor().show(i);
    }

    pub fn current_page(&self) -> Page {
        let pages = self.sheet.pages();
        pages[self.page.min(pages.len() - 1)]
    }

    /// Shows page `index`, the focus on its first control (as a click on a tab or Ctrl+Tab does).
    pub fn show_page(&mut self, index: usize) {
        self.page = index.min(self.sheet.pages().len() - 1);
        self.focus = first_control(self.era, self.current_page(), &self.values);
        self.drop = None;
    }

    /// Whether `part` can be used now: Apply only after a change, Clear only while the
    /// Documents menu lists something, Use the PC's Time only while Bevel95's clock differs;
    /// Personalized Menus never.
    pub fn enabled(&self, part: Part) -> bool {
        match part {
            Part::Apply => self.changed,
            Part::Clear => !self.documents_empty,
            Part::UsePcTime => self.values.clock_offset != 0,
            Part::Check(Check::PersonalizedMenus) => false,
            // How a size is drawn matters only between whole numbers.
            Part::WayList => !self.values.scale.is_whole(),
            _ => true,
        }
    }

    /// "Use the PC's Time": Bevel95's clock goes back to the PC's time (on OK or Apply, like
    /// everything else here). Returns false if it already shows it.
    pub fn use_pc_time(&mut self) -> bool {
        if self.values.clock_offset == 0 {
            return false;
        }
        self.values.clock_offset = 0;
        self.changed = true;
        true
    }

    /// The push button Enter presses: the focused one, else OK.
    pub fn default_button(&self) -> Part {
        if self.focus.is_push_button() && self.enabled(self.focus) { self.focus } else { Part::Ok }
    }

    /// Flips a check box, picks an option button or flips a row of a list (Start Menu Settings,
    /// the programs); returns false for any other part (or one that cannot change).
    pub fn choose(&mut self, part: Part) -> bool {
        if !self.enabled(part) {
            return false;
        }
        let v = &mut self.values;
        match part {
            Part::Check(c) => {
                let b = check_value(v, c);
                *b = !*b;
            }
            Part::Look(era) => v.era = era,
            Part::Colors(depth) => v.colors = depth,
            Part::WinKey(key) => v.windows_key = key,
            Part::Shown(shown) => {
                v.programs = shown;
                // The list shows what the choice is about: the classic programs' folders open
                // (and no others), with the first of those programs in view.
                programs::reveal(&mut self.programs.rows, shown);
                let shows = programs::visible(&self.programs.rows);
                if !shows.contains(&self.programs.sel) {
                    self.programs.sel = 0;
                }
                self.programs.top = self.programs.top.min(shows.len().saturating_sub(PROGRAM_PAGE));
                if shown == Shown::Classic
                    && let Some(first) = self.programs.rows.iter().position(|row| row.own)
                {
                    self.show_program(first);
                }
            }
            Part::ProgramRow(i) => {
                if i >= self.programs.rows.len() {
                    return false;
                }
                // The first box flipped while another choice is made: what that choice shows
                // becomes the ticked programs (the classic ones as a base to add to), and the
                // choice becomes "Only the programs ticked here".
                if v.programs != Shown::Ticked {
                    programs::adopt(&mut self.programs.rows, v.programs);
                    v.programs = Shown::Ticked;
                }
                programs::toggle(&mut self.programs.rows, i);
                self.programs.edited = true;
            }
            Part::ListRow(i) => {
                let Some(b) = SETTINGS.get(i).and_then(|&s| setting_value(&mut v.start, s)) else { return false };
                *b = !*b;
            }
            _ => return false,
        }
        self.changed = true;
        true
    }

    /// Whether check box `c` is ticked.
    pub fn is_checked(&self, c: Check) -> bool {
        let mut v = self.values;
        *check_value(&mut v, c)
    }

    /// Whether a Start Menu Settings entry is ticked.
    pub fn is_on(&self, s: Setting) -> bool {
        let mut start = self.values.start;
        setting_value(&mut start, s).is_some_and(|b| *b)
    }

    /// The parts the Tab key goes through, in order: the tabs (one stop), the page's controls
    /// (a group of option buttons is one stop: its chosen button), then OK, Cancel, Apply.
    pub fn tab_stops(&self) -> Vec<Part> {
        let mut stops = vec![Part::Tab(self.page)];
        for part in controls(self.era, self.current_page()) {
            let stop = match part {
                Part::Look(_) => Part::Look(self.values.era),
                Part::Colors(_) => Part::Colors(self.values.colors),
                Part::WinKey(_) => Part::WinKey(self.values.windows_key),
                Part::Shown(_) => Part::Shown(self.values.programs),
                p => p,
            };
            if !stops.contains(&stop) && self.enabled(stop) {
                stops.push(stop);
            }
        }
        stops.extend([Part::Ok, Part::Cancel].into_iter().chain(self.enabled(Part::Apply).then_some(Part::Apply)));
        stops
    }

    /// The control of the current page whose access key is `ch` (Apply too).
    pub fn access_key(&self, ch: char) -> Option<Part> {
        let ch = ch.to_ascii_lowercase();
        let parts = controls(self.era, self.current_page()).into_iter().chain([Part::Apply]);
        parts.filter(|&p| self.enabled(p)).find(|&p| {
            label_of(self.era, p).is_some_and(|label| {
                let (text, key) = split_access_key(&label);
                key.and_then(|k| text.chars().nth(k)).is_some_and(|c| c.to_ascii_lowercase() == ch)
            })
        })
    }

    /// The option button an arrow key moves to from `part` (within its group), if any.
    pub fn arrow(&self, part: Part, forward: bool) -> Option<Part> {
        let group: Vec<Part> = match part {
            Part::Look(_) => Era::ALL.iter().map(|&e| Part::Look(e)).collect(),
            Part::Colors(_) => Depth::ALL.iter().map(|&d| Part::Colors(d)).collect(),
            Part::WinKey(_) => WindowsKey::ALL.iter().map(|&k| Part::WinKey(k)).collect(),
            Part::Shown(_) => Shown::ALL.iter().map(|&s| Part::Shown(s)).collect(),
            _ => return None,
        };
        let at = group.iter().position(|&p| p == part)?;
        let next = if forward { (at + 1).min(group.len() - 1) } else { at.saturating_sub(1) };
        Some(group[next])
    }

    /// Moves the list's selection by `delta` rows, scrolling to keep it in view.
    pub fn move_selection(&mut self, delta: i32) {
        let last = SETTINGS.len() as i32 - 1;
        self.list_sel = (self.list_sel as i32 + delta).clamp(0, last) as usize;
        if self.list_sel < self.list_top {
            self.list_top = self.list_sel;
        } else if self.list_sel >= self.list_top + LIST_PAGE {
            self.list_top = self.list_sel + 1 - LIST_PAGE;
        }
    }

    /// Scrolls the list of the page shown by `delta` rows.
    pub fn scroll(&mut self, delta: i32) {
        let (total, page) = self.list_size();
        let max = total.saturating_sub(page) as i32;
        let top = (self.list_top() as i32 + delta).clamp(0, max) as usize;
        self.set_list_top(top);
    }

    /// The list of the page shown (a page has one at most; Windows 2000's Start Menu Settings
    /// unless it is the Programs page): how many rows it has and how many of them show.
    fn list_size(&self) -> (usize, usize) {
        match self.current_page() {
            Page::Programs => (self.program_rows().len(), PROGRAM_PAGE),
            _ => (SETTINGS.len(), LIST_PAGE),
        }
    }

    /// The first row that list shows.
    fn list_top(&self) -> usize {
        if self.current_page() == Page::Programs { self.programs.top } else { self.list_top }
    }

    fn set_list_top(&mut self, top: usize) {
        if self.current_page() == Page::Programs {
            self.programs.top = top;
        } else {
            self.list_top = top;
        }
    }

    /// That list's scroll bar, if it has one now (the list of programs has none while all of
    /// its rows show).
    fn scroll_bar(&self) -> Option<ScrollBar> {
        match self.current_page() {
            Page::Programs => programs_scroll(self),
            Page::StartMenuPrograms if is_2000(self.era) => Some(settings_scroll()),
            _ => None,
        }
    }

    /// Moves the selection in the list of programs by `delta` of the rows that show, scrolling
    /// to keep it in view.
    pub fn move_program(&mut self, delta: i32) {
        self.cursor().move_by(delta);
    }

    /// A letter typed in the list of programs: the selection goes to the next row that shows
    /// whose name starts with it, around the end of the list (as Windows' lists do). False if
    /// none does.
    pub fn find_program(&mut self, ch: char) -> bool {
        self.cursor().find(ch)
    }

    /// The Right key in the list of programs: a closed folder opens; an open one passes the
    /// selection to its first row. The Left key: an open folder closes; any other row passes
    /// the selection to the folder it is in. As in Windows' trees.
    pub fn step_program(&mut self, right: bool) {
        self.cursor().step(right);
    }

    /// The "*" key in the list of programs: the selected folder opens with every folder in it.
    pub fn open_all_programs(&mut self) {
        self.cursor().open_all();
    }
}

fn check_value(v: &mut Values, c: Check) -> &mut bool {
    match c {
        Check::AlwaysOnTop => &mut v.options.always_on_top,
        Check::AutoHide => &mut v.options.auto_hide,
        Check::SmallIcons => &mut v.options.small_icons,
        Check::ShowClock => &mut v.options.show_clock,
        // Never changes (always grayed out); this just gives it somewhere to point.
        Check::PersonalizedMenus => &mut v.start_with_windows,
        Check::StartWithWindows => &mut v.start_with_windows,
        Check::Desktop => &mut v.desktop,
        Check::Group => &mut v.group,
        Check::QuickSearchAll => &mut v.quick_search_all,
        Check::BatteryPercent => &mut v.battery_percent,
        Check::QuickLaunch => &mut v.quick_launch,
    }
}

fn setting_value(start: &mut StartOptions, s: Setting) -> Option<&mut bool> {
    match s {
        Setting::Favorites => Some(&mut start.favorites),
        Setting::LogOff => Some(&mut start.log_off),
        _ => None,
    }
}

/// The controls of a page, in Tab order.
fn controls(era: Era, page: Page) -> Vec<Part> {
    match page {
        Page::TaskbarOptions => option_checks(era).iter().map(|&(c, ..)| Part::Check(c)).collect(),
        Page::StartMenuPrograms if is_2000(era) => {
            vec![Part::Add, Part::Remove, Part::Advanced, Part::ReSort, Part::Clear, Part::List]
        }
        Page::StartMenuPrograms => vec![Part::Add, Part::Remove, Part::Advanced, Part::Clear],
        Page::Bevel95 => Era::ALL
            .iter()
            .map(|&e| Part::Look(e))
            .chain(Depth::ALL.iter().map(|&d| Part::Colors(d)))
            .chain([Part::SizeList, Part::WayList])
            .chain([Part::Check(Check::StartWithWindows), Part::UsePcTime])
            .collect(),
        Page::OwnTaskbar => {
            WindowsKey::ALL
                .iter()
                .map(|&k| Part::WinKey(k))
                .chain([
                    Part::Check(Check::Group),
                    Part::Check(Check::QuickLaunch),
                    Part::Check(Check::QuickSearchAll),
                    Part::Check(Check::BatteryPercent),
                ])
                .collect()
        }
        Page::Desktop => vec![Part::Check(Check::Desktop)],
        Page::Programs => Shown::ALL.iter().map(|&s| Part::Shown(s)).chain([Part::Programs]).collect(),
    }
}

fn first_control(era: Era, page: Page, values: &Values) -> Part {
    match page {
        Page::Bevel95 => Part::Look(values.era),
        Page::OwnTaskbar => Part::WinKey(values.windows_key),
        Page::Programs => Part::Shown(values.programs),
        p => controls(era, p)[0],
    }
}

/// Where everything goes, at 1×.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertiesLayout {
    pub width: i32,
    pub height: i32,
    pub title: &'static str,
    /// Each page's tab where it sits when not chosen (the chosen one is drawn 2px larger).
    pub tabs: Vec<Rect>,
    /// The frame around the page.
    pub page: Rect,
    /// What takes clicks: the page's controls, OK, Cancel, Apply, the tabs, the title buttons.
    pub parts: Vec<(Part, Rect)>,
}

impl PropertiesLayout {
    pub fn rect(&self, part: Part) -> Option<Rect> {
        self.parts.iter().find(|(p, _)| *p == part).map(|&(_, r)| r)
    }

    /// The part under (x, y).
    pub fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.parts.iter().find(|(_, r)| r.contains(x, y)).map(|&(p, _)| p)
    }
}

// The window, the same in every version: 344×400, tabs from x 11 on a row 18 tall at y 31
// (a tab is its label plus 6 on each side), the page's frame from (9, 49), OK, Cancel and
// Apply 75×23 at y 367.
const SIZE: (i32, i32) = (344, 400);
const TAB_X: i32 = 11;
const TAB_Y: i32 = 31;
const TAB_H: i32 = 18;
const TAB_PAD: i32 = 6;
const PAGE: Rect = Rect::new(9, 49, 326, 312);
const BUTTONS: [(Part, i32); 3] = [(Part::Ok, 98), (Part::Cancel, 179), (Part::Apply, 260)];
const BUTTONS_Y: i32 = 367;

/// Taskbar Options: the preview's frame, and the check boxes' top-left corners.
const PREVIEW: Rect = Rect::new(34, 74, 276, 132);
fn option_checks(era: Era) -> &'static [(Check, i32, i32)] {
    if is_2000(era) {
        &[
            (Check::AlwaysOnTop, 34, 226),
            (Check::AutoHide, 34, 252),
            (Check::SmallIcons, 34, 276),
            (Check::ShowClock, 34, 301),
            (Check::PersonalizedMenus, 34, 325),
        ]
    } else {
        &[
            (Check::AlwaysOnTop, 34, 226),
            (Check::AutoHide, 34, 257),
            (Check::SmallIcons, 34, 288),
            (Check::ShowClock, 34, 319),
        ]
    }
}
/// A check box's label starts this far right of the box.
const CHECK_TEXT_X: i32 = 18;

/// Start Menu Programs (95, 98): the two groups, the buttons.
const CUSTOMIZE_GROUP: Rect = Rect::new(24, 68, 296, 135);
const DOCUMENTS_GROUP: Rect = Rect::new(24, 215, 296, 101);
const PAGE_BUTTONS: [(Part, Rect); 4] = [
    (Part::Add, Rect::new(48, 166, 78, 23)),
    (Part::Remove, Rect::new(132, 166, 80, 23)),
    (Part::Advanced, Rect::new(217, 166, 78, 23)),
    (Part::Clear, Rect::new(217, 280, 78, 23)),
];

/// Advanced (2000): the group, its buttons, the list of settings (16px rows inside a sunken
/// frame, a 16px scroll bar at its right).
const ADVANCED_GROUP: Rect = Rect::new(25, 68, 296, 181);
const ADVANCED_BUTTONS: [(Part, Rect); 5] = [
    (Part::Add, Rect::new(229, 80, 80, 23)),
    (Part::Remove, Rect::new(229, 105, 80, 23)),
    (Part::Advanced, Rect::new(229, 131, 80, 23)),
    (Part::ReSort, Rect::new(229, 157, 80, 23)),
    (Part::Clear, Rect::new(229, 217, 80, 23)),
];
const LIST: Rect = Rect::new(24, 272, 297, 73);
const LIST_ROW_H: i32 = 16;
/// Rows the list shows in full (the scroll bar's page).
const LIST_PAGE: usize = 4;

fn list_inside() -> Rect {
    LIST.inset(2, 2, 2, 2)
}

fn scroll_bar_rect() -> Rect {
    let inside = list_inside();
    Rect::new(inside.right() - 16, inside.y, 16, inside.h)
}

/// A scroll bar's parts as this window's (a page has one list at most), while `top` is the
/// first row shown.
fn scroll_parts(scroll: ScrollBar, top: usize) -> [(Part, Rect); 5] {
    let r = scroll.rects(top);
    [
        (Part::ScrollUp, r.up),
        (Part::ScrollDown, r.down),
        (Part::ScrollThumb, r.thumb),
        (Part::ScrollPageUp, r.page_up),
        (Part::ScrollPageDown, r.page_down),
    ]
}

/// Start Menu Settings' scroll bar.
fn settings_scroll() -> ScrollBar {
    ScrollBar { bar: scroll_bar_rect(), total: SETTINGS.len(), page: LIST_PAGE }
}

/// Its thumb while row `top` is the first one shown.
#[cfg(test)]
fn thumb_rect(top: usize) -> Rect {
    settings_scroll().thumb(top)
}

fn list_row_rect(i: usize, top: usize) -> Rect {
    let inside = list_inside();
    Rect::new(inside.x, inside.y + (i as i32 - top as i32) * LIST_ROW_H, inside.w - 16, LIST_ROW_H)
}

/// Bevel95 Settings' page (ours): the groups (8 apart), the option rows (top-left, as in the Shut Down
/// dialog: the button 2px down, its label 17px right), the check box, and the Clock group's
/// two lines of text with its button at the right (12 in from the group's edge, as on
/// Windows 2000's Advanced page).
/// The Look group and, beside it, the Colors group (each half of the row, 8 apart).
const LOOK_GROUP: Rect = Rect::new(24, 68, 144, 89);
const COLORS_GROUP: Rect = Rect::new(176, 68, 144, 89);
const COLORS_ROWS: [(Depth, i32, i32); 3] =
    [(Depth::Sixteen, 188, 84), (Depth::TwoFiftySix, 188, 105), (Depth::TrueColor, 188, 126)];
const LOOK_ROWS: [(Era, i32, i32); 3] = [(Era::Win95, 36, 84), (Era::Win98, 36, 105), (Era::Win2000, 36, 126)];
const SIZE_GROUP: Rect = Rect::new(24, 165, 296, 68);
/// The Size group's row (ours, PROVISIONAL): "Size:" and its drop-down list, "Drawn as:" and
/// its list (21 high, as Run's and Shut Down's fields), the labels on the fields' text line; then
/// a line of text.
const SIZE_TEXT_Y: i32 = 183;
const SIZE_LABEL_X: i32 = 36;
const SIZE_FIELD: Rect = Rect::new(64, 179, 58, 21);
const WAY_LABEL_X: i32 = 132;
const WAY_FIELD: Rect = Rect::new(180, 179, 128, 21);
const SIZE_LINE_Y: i32 = 206;
/// An open list's rows (as Shut Down's in Windows 2000).
const DROP_ROW_H: i32 = 13;

/// Where drop-down list `list` opens: under its field, at least as wide as the field and as
/// its widest entry, kept inside the window.
fn drop_frame(font: &Font, list: Part) -> Option<Rect> {
    let field = match list {
        Part::SizeList => SIZE_FIELD,
        Part::WayList => WAY_FIELD,
        _ => return None,
    };
    let choices = list_choices(list);
    let widest = choices.iter().map(|c| font.measure(c)).max().unwrap_or(0) + 6;
    let w = field.w.max(widest);
    let x = field.x.min(SIZE.0 - 4 - w);
    Some(Rect::new(x, field.bottom(), w, choices.len() as i32 * DROP_ROW_H + 2))
}
const STARTUP_GROUP: Rect = Rect::new(24, 241, 296, 47);
const STARTUP_CHECK: (i32, i32) = (36, 258);
const CLOCK_GROUP: Rect = Rect::new(24, 296, 296, 55);
const CLOCK_BUTTON_Y: i32 = 312;

/// Bevel95 Settings' Taskbar page (ours), in the General page's manner: the Windows key group
/// (its two option rows as the looks' first two, the group as high as the Size group), then 8
/// below it the Buttons group with its check box (as Startup's) and a line of text (as Size's).
const KEY_GROUP: Rect = Rect::new(24, 68, 296, 68);
const KEY_ROWS: [(WindowsKey, i32, i32); 2] = [(WindowsKey::Find, 36, 84), (WindowsKey::Start, 36, 105)];
/// The Buttons group holds Quick Launch's check box too, 21 below the line of text (PROVISIONAL:
/// ours), and is 21 taller for it.
const BUTTONS_GROUP: Rect = Rect::new(24, 144, 296, 90);
const GROUP_CHECK: (i32, i32) = (36, 161);
const GROUP_TEXT: &str = "Off: every window has a button of its own.";
const QUICK_LAUNCH_CHECK: (i32, i32) = (36, 204);
/// 8 below the Buttons group, the Quick Search group (as high as Startup on the General page),
/// its check box as Startup's.
const SEARCH_GROUP: Rect = Rect::new(24, 242, 296, 47);
const SEARCH_CHECK: (i32, i32) = (36, 259);
/// 8 below the Quick Search group, the Battery group, as high, its check box as Startup's.
const BATTERY_GROUP: Rect = Rect::new(24, 297, 296, 47);
const BATTERY_CHECK: (i32, i32) = (36, 314);

/// Bevel95 Settings' Desktop page (ours): one group in the General page's manner, its check box
/// and three lines saying what the desktop is.
const DESKTOP_GROUP: Rect = Rect::new(24, 68, 296, 88);
const DESKTOP_CHECK: (i32, i32) = (36, 85);
const DESKTOP_TEXT: [&str; 3] = [
    "A plain desktop in the look's own color.",
    "Windows' own desktop stays underneath it,",
    "with your icons where you left them.",
];

/// Bevel95 Settings' Programs page (ours): one group down to where the General page ends, the
/// three option rows as the General page has its looks, and under them the list of programs,
/// 12 in from the group's edges: a sunken frame around a tree of 12 rows (`tree`: Windows' own
/// tree of the Programs menu, with a check box added to each row), a 16px scroll bar at its
/// right while more rows show than fit.
const PROGRAMS_GROUP: Rect = Rect::new(24, 68, 296, 283);
const SHOWN_ROWS: [(Shown, i32, i32); 3] =
    [(Shown::Everything, 36, 84), (Shown::Classic, 36, 105), (Shown::Ticked, 36, 126)];
const PROGRAMS_LIST: Rect = Rect::new(36, 147, 272, 196);
/// Rows the list of programs shows (the scroll bar's page).
const PROGRAM_PAGE: usize = 12;
/// What the list says until its rows are read.
const PROGRAMS_WAIT: &str = "Looking for programs...";

fn programs_inside() -> Rect {
    PROGRAMS_LIST.inset(2, 2, 2, 2)
}

/// The list of programs' scroll bar: none while every row that shows fits.
fn programs_scroll(p: &Properties) -> Option<ScrollBar> {
    let (inside, total) = (programs_inside(), p.program_rows().len());
    let bar = Rect::new(inside.right() - 16, inside.y, 16, inside.h);
    (total > PROGRAM_PAGE).then_some(ScrollBar { bar, total, page: PROGRAM_PAGE })
}

/// The width of the list's rows: its inside, less the scroll bar while it has one.
fn program_rows_width(p: &Properties) -> i32 {
    programs_inside().w - if programs_scroll(p).is_some() { 16 } else { 0 }
}

/// The Clock group's button: its label plus 12 on each side, at least Windows' usual 75.
fn clock_button(font: &Font, era: Era) -> Rect {
    let w = (font.measure_label(button_label(era, Part::UsePcTime)) + 24).max(75);
    Rect::new(CLOCK_GROUP.right() - 12 - w, CLOCK_BUTTON_Y, w, 23)
}

/// What the Clock group says about Bevel95's clock, in two lines.
pub fn clock_lines(offset: i64) -> [String; 2] {
    if offset == 0 {
        return ["Bevel95's clock shows".to_string(), "the PC's time.".to_string()];
    }
    let way = if offset > 0 { "ahead of" } else { "behind" };
    ["Bevel95's clock runs".to_string(), format!("{} {way} the PC.", amount(offset.unsigned_abs()))]
}

/// A length of time in words, in its largest unit, rounded: "5 minutes", "about 1 hour".
fn amount(seconds: u64) -> String {
    const UNITS: [(u64, &str); 5] =
        [(365 * 86_400, "year"), (86_400, "day"), (3_600, "hour"), (60, "minute"), (1, "second")];
    let mut i = UNITS.iter().position(|&(size, _)| seconds >= size).unwrap_or(UNITS.len() - 1);
    let mut n = (seconds + UNITS[i].0 / 2) / UNITS[i].0;
    if i > 0 && n * UNITS[i].0 >= UNITS[i - 1].0 {
        // Rounded up into the next unit: 59 minutes 50 seconds is about 1 hour.
        i -= 1;
        n = 1;
    }
    let (size, name) = UNITS[i];
    let about = if n * size == seconds { "" } else { "about " };
    let plural = if n == 1 { "" } else { "s" };
    format!("{about}{n} {name}{plural}")
}

pub fn layout(p: &Properties, fonts: &Fonts) -> PropertiesLayout {
    let font = fonts.regular;
    let mut tabs = Vec::new();
    let mut x = TAB_X;
    for &page in p.sheet.pages() {
        let w = font.measure(page_title(p.era, page)) + 2 * TAB_PAD;
        tabs.push(Rect::new(x, TAB_Y, w, TAB_H));
        x += w;
    }
    let label_w = |part: Part| label_of(p.era, part).map_or(0, |l| font.measure_label(&l));
    let check = |part: Part, x: i32, y: i32| (part, Rect::new(x, y - 1, CHECK_TEXT_X + label_w(part) + 1, 16));
    let mut parts: Vec<(Part, Rect)> = Vec::new();
    match p.current_page() {
        Page::TaskbarOptions => {
            for &(c, x, y) in option_checks(p.era) {
                parts.push(check(Part::Check(c), x, y));
            }
        }
        Page::StartMenuPrograms if is_2000(p.era) => {
            parts.extend(ADVANCED_BUTTONS);
            let inside = list_inside();
            for i in 0..SETTINGS.len() {
                let r = list_row_rect(i, p.list_top);
                let top = r.y.max(inside.y);
                let bottom = r.bottom().min(inside.bottom());
                if bottom > top {
                    parts.push((Part::ListRow(i), Rect::new(r.x, top, r.w, bottom - top)));
                }
            }
            parts.extend(scroll_parts(settings_scroll(), p.list_top));
            parts.push((Part::List, LIST));
        }
        Page::StartMenuPrograms => parts.extend(PAGE_BUTTONS),
        Page::Bevel95 => {
            for (era, x, y) in LOOK_ROWS {
                let part = Part::Look(era);
                parts.push((part, Rect::new(x, y, 17 + label_w(part) + 1, 16)));
            }
            for (depth, x, y) in COLORS_ROWS {
                let part = Part::Colors(depth);
                parts.push((part, Rect::new(x, y, 17 + label_w(part) + 1, 16)));
            }
            // An open list lies over everything under it: its rows come first.
            if let Some((list, _)) = p.drop
                && let Some(frame) = drop_frame(font, list)
            {
                for i in 0..list_choices(list).len() {
                    let row = Rect::new(frame.x + 1, frame.y + 1 + i as i32 * DROP_ROW_H, frame.w - 2, DROP_ROW_H);
                    parts.insert(i, (Part::DropRow(i), row));
                }
            }
            parts.push((Part::SizeList, SIZE_FIELD));
            parts.push((Part::WayList, WAY_FIELD));
            parts.push(check(Part::Check(Check::StartWithWindows), STARTUP_CHECK.0, STARTUP_CHECK.1));
            parts.push((Part::UsePcTime, clock_button(font, p.era)));
        }
        Page::OwnTaskbar => {
            for (key, x, y) in KEY_ROWS {
                let part = Part::WinKey(key);
                parts.push((part, Rect::new(x, y, 17 + label_w(part) + 1, 16)));
            }
            parts.push(check(Part::Check(Check::Group), GROUP_CHECK.0, GROUP_CHECK.1));
            parts.push(check(Part::Check(Check::QuickLaunch), QUICK_LAUNCH_CHECK.0, QUICK_LAUNCH_CHECK.1));
            parts.push(check(Part::Check(Check::QuickSearchAll), SEARCH_CHECK.0, SEARCH_CHECK.1));
            parts.push(check(Part::Check(Check::BatteryPercent), BATTERY_CHECK.0, BATTERY_CHECK.1));
        }
        Page::Desktop => parts.push(check(Part::Check(Check::Desktop), DESKTOP_CHECK.0, DESKTOP_CHECK.1)),
        Page::Programs => {
            for (shown, x, y) in SHOWN_ROWS {
                let part = Part::Shown(shown);
                parts.push((part, Rect::new(x, y, 17 + label_w(part) + 1, 16)));
            }
            let (inside, top, w) = (programs_inside(), p.programs.top, program_rows_width(p));
            for (at, &row) in p.program_rows().iter().enumerate().skip(top).take(PROGRAM_PAGE) {
                let y = inside.y + (at - top) as i32 * tree::ROW_H;
                parts.push((Part::ProgramRow(row), Rect::new(inside.x, y, w, tree::ROW_H)));
            }
            if let Some(scroll) = programs_scroll(p) {
                parts.extend(scroll_parts(scroll, top));
            }
            parts.push((Part::Programs, PROGRAMS_LIST));
        }
    }
    for (part, x) in BUTTONS {
        parts.push((part, Rect::new(x, BUTTONS_Y, 75, 23)));
    }
    for (i, &t) in tabs.iter().enumerate() {
        let r = if i == p.page { Rect::new(t.x - 2, t.y - 2, t.w + 4, t.h + 2) } else { t };
        parts.push((Part::Tab(i), r));
    }
    let (help, close) = dialog::title_button_rects(SIZE.0);
    parts.extend([(Part::TitleHelp, help), (Part::TitleClose, close)]);
    PropertiesLayout { width: SIZE.0, height: SIZE.1, title: title(p.sheet, p.era), tabs, page: PAGE, parts }
}

/// A picture cut from the emulator's screen: raw RGB, row by row.
struct Picture {
    w: i32,
    h: i32,
    rgb: &'static [u8],
}

impl Picture {
    fn paint(&self, fb: &mut Framebuffer, x: i32, y: i32) {
        for j in 0..self.h {
            for i in 0..self.w {
                let k = ((j * self.w + i) * 3) as usize;
                let p = &self.rgb[k..k + 3];
                fb.set(x + i, y + j, rgb(u32::from_be_bytes([0, p[0], p[1], p[2]])));
            }
        }
    }
}

macro_rules! picture {
    ($w:expr, $h:expr, $what:literal, $look:ident) => {
        Picture { w: $w, h: $h, rgb: crate::art::bytes($what, crate::art::Look::$look) }
    };
}

/// One design's preview pieces.
struct PreviewPictures {
    window: Picture,
    start_large: Picture,
    start_small: Picture,
    taskbar: Picture,
    taskbar_no_clock: Picture,
}

/// Plain Windows 95's (its EXPLORER.EXE without Internet Explorer 4): its own Start menus and
/// taskbar; the window is the same as 98's.
static W95_PREVIEW: PreviewPictures = PreviewPictures {
    window: picture!(62, 117, "properties/preview-window", Win95),
    start_large: picture!(152, 99, "properties/preview-start-large", Win95),
    start_small: picture!(115, 99, "properties/preview-start-small", Win95),
    taskbar: picture!(269, 29, "properties/preview-taskbar", Win95),
    taskbar_no_clock: picture!(269, 29, "properties/preview-taskbar-no-clock", Win95),
};
static IE4_PREVIEW: PreviewPictures = PreviewPictures {
    window: picture!(62, 117, "properties/preview-window", Win98),
    start_large: picture!(152, 99, "properties/preview-start-large", Win98),
    start_small: picture!(115, 99, "properties/preview-start-small", Win98),
    taskbar: picture!(269, 29, "properties/preview-taskbar", Win98),
    taskbar_no_clock: picture!(269, 29, "properties/preview-taskbar-no-clock", Win98),
};
static W2K_PREVIEW: PreviewPictures = PreviewPictures {
    window: picture!(62, 117, "properties/preview-window", Win2000),
    start_large: picture!(152, 99, "properties/preview-start-large", Win2000),
    start_small: picture!(115, 99, "properties/preview-start-small", Win2000),
    taskbar: picture!(269, 29, "properties/preview-taskbar", Win2000),
    taskbar_no_clock: picture!(269, 29, "properties/preview-taskbar-no-clock", Win2000),
};
static IE4_CUSTOMIZE_ICON: Picture = picture!(32, 32, "properties/customize-start", Win98);
static IE4_DOCUMENTS_ICON: Picture = picture!(32, 32, "properties/documents", Win98);
static W2K_CUSTOMIZE_ICON: Picture = picture!(32, 32, "properties/customize-start", Win2000);
static W2K_CLEAR_ICON: Picture = picture!(32, 32, "properties/clear", Win2000);

/// The preview of the Taskbar Options page for `o`, in the frame `frame`. Windows draws the
/// desktop, the sample window and the taskbar (the taskbar last when it is always on top,
/// else the window over it), then the Start menu; a hidden taskbar becomes a 4px strip with
/// a dotted cyan outline where it would be.
fn paint_preview(fb: &mut Framebuffer, theme: &Theme, frame: Rect, o: TaskbarOptions) {
    let (era, c) = (theme.era, &theme.colors);
    let pics = match era {
        Era::Win95 => &W95_PREVIEW,
        _ if is_2000(era) => &W2K_PREVIEW,
        _ => &IE4_PREVIEW,
    };
    let inner = frame.inset(1, 1, 1, 1);
    let mut pic = Framebuffer::new(inner.w, inner.h, rgb(0x008080));
    // The pieces' places, from the inside top-left corner of the frame.
    let window = (212, 0);
    let taskbar = (2, 100);
    let start = (2, 0);
    let draw_taskbar = |pic: &mut Framebuffer| {
        if !o.auto_hide {
            let t = if o.show_clock { &pics.taskbar } else { &pics.taskbar_no_clock };
            t.paint(pic, taskbar.0, taskbar.1);
        }
    };
    if o.always_on_top {
        pics.window.paint(&mut pic, window.0, window.1);
        draw_taskbar(&mut pic);
    } else {
        draw_taskbar(&mut pic);
        pics.window.paint(&mut pic, window.0, window.1);
    }
    let menu = if o.small_icons { &pics.start_small } else { &pics.start_large };
    menu.paint(&mut pic, start.0, start.1);
    if o.auto_hide {
        let (x0, x1, y0) = (taskbar.0, taskbar.0 + 268, taskbar.1);
        let cyan = rgb(0x00FFFF);
        for x in (x0..=x1).step_by(2) {
            pic.set(x, y0, cyan);
        }
        for y in (y0 + 2..y0 + 25).step_by(2) {
            pic.set(x0, y, cyan);
            pic.set(x1, y, cyan);
        }
        // The hidden taskbar's top 4 rows: its raised edge (black even in Windows 2000), no
        // bottom.
        let y = y0 + 25;
        pic.fill(Rect::new(x0, y, 269, 4), c.face);
        pic.hline(x0, y, 268, c.highlight);
        pic.vline(x0, y + 1, 3, c.highlight);
        pic.vline(x1 - 1, y + 1, 3, c.shadow);
        pic.vline(x1, y, 4, rgb(0x000000));
    }
    // In 16 colors the pieces cut from the True Color captures take the look's rule (Windows 2000's
    // face D4D0C8 in them becomes silver, as its system color did; at 256 colors it keeps its face
    // and desktop, `colors256::area_pixel`).
    if theme.depth.reduced() {
        for p in &mut pic.pixels {
            *p = crate::colors256::area_pixel(theme, *p);
        }
    }
    fb.blit(&pic, inner.x, inner.y);
    fb.frame(frame, c.shadow, c.highlight);
}

/// A check box (13×13, Windows' sunken box with a white inside, gray while held down or
/// grayed out) at (x, y), with Windows 95's check mark.
pub(crate) fn check_box(fb: &mut Framebuffer, c: &Colors, x: i32, y: i32, checked: bool, pressed: bool, enabled: bool) {
    fb.frame(Rect::new(x, y, 13, 13), c.shadow, c.highlight);
    fb.frame(Rect::new(x + 1, y + 1, 11, 11), c.dark_shadow, c.light);
    fb.fill(Rect::new(x + 2, y + 2, 9, 9), if pressed || !enabled { c.face } else { rgb(0xFFFFFF) });
    if checked {
        let ink = if enabled { c.text } else { c.shadow };
        dialog::art(fb, CHECK_ART, x + 3, y + 3, |b| (b == b'#').then_some(ink));
    }
}


/// A group box: an etched frame with its label set into the top line (its access key, if any,
/// underlined while `keys`). Date/Time Properties' groups too.
pub(crate) fn group_box(fb: &mut Framebuffer, c: &Colors, font: &Font, r: Rect, label: &str, keys: bool) {
    fb.frame(r, c.shadow, c.highlight);
    fb.frame(r.inset(1, 1, 1, 1), c.highlight, c.shadow);
    let text_x = r.x + 9;
    fb.fill(Rect::new(text_x - 2, r.y, font.measure_label(label) + 4, 2), c.face);
    font.draw_label(fb, (text_x, r.y - 6), label, i32::MAX, c.text, keys);
}

/// A tab, drawn at `r`: raised, with Windows' cut-off top corners and no bottom edge.
pub(crate) fn tab(fb: &mut Framebuffer, c: &Colors, r: Rect) {
    let (x, y, w, h) = (r.x, r.y, r.w, r.h);
    fb.fill(Rect::new(x + 1, y + 1, w - 2, h - 1), c.face);
    fb.hline(x + 2, y, w - 4, c.highlight);
    fb.set(x + 1, y + 1, c.highlight);
    fb.vline(x, y + 2, h - 2, c.highlight);
    fb.hline(x + 2, y + 1, w - 4, c.light);
    fb.vline(x + 1, y + 2, h - 2, c.light);
    fb.set(x + w - 2, y + 1, c.dark_shadow);
    fb.vline(x + w - 1, y + 2, h - 2, c.dark_shadow);
    fb.vline(x + w - 2, y + 2, h - 2, c.shadow);
}

/// A scroll bar button (16×16) with its 7×4 arrow, as Windows 2000's list shows it; pushed in
/// (PROVISIONAL): a flat shadow edge, the arrow 1px down-right.
pub(crate) fn scroll_arrow(fb: &mut Framebuffer, c: &Colors, r: Rect, up: bool, pressed: bool) {
    fb.fill(r, c.face);
    if pressed {
        fb.frame(r, c.shadow, c.shadow);
    } else {
        fb.frame(r, c.light, c.dark_shadow);
        fb.frame(r.inset(1, 1, 1, 1), c.highlight, c.shadow);
    }
    let s = i32::from(pressed);
    for row in 0..4 {
        let w = if up { 2 * row + 1 } else { 7 - 2 * row };
        fb.hline(r.x + 7 - w / 2 + s, r.y + 6 + row + s, w, c.text);
    }
}

/// Paints the whole window filling the framebuffer (`layout`'s size).
pub fn paint(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, p: &Properties, l: &PropertiesLayout) {
    let c = &theme.colors;
    let font = fonts.regular;
    let keys = p.show_keys;
    let era = p.era;
    let held = |part: Part| p.pressed == Some(part);
    let buttons = TitleButtons {
        help: l.rect(Part::TitleHelp).map(|r| (r, held(Part::TitleHelp))),
        close: l.rect(Part::TitleClose).map(|r| (r, held(Part::TitleClose))),
    };
    dialog::window_frame(fb, theme, fonts, era, l.title, &buttons);

    // The tabs not chosen, the page's frame (raised like a button), then the chosen tab over
    // the frame's top line, so that it runs into the page.
    let tab_text = |fb: &mut Framebuffer, i: usize, top: i32| {
        font.draw(fb, l.tabs[i].x + TAB_PAD, top + 3, page_title(era, p.sheet.pages()[i]), c.text);
    };
    for (i, &t) in l.tabs.iter().enumerate() {
        if i != p.page {
            tab(fb, c, t);
            tab_text(fb, i, t.y);
        }
    }
    fb.frame(l.page, c.highlight, c.dark_shadow);
    fb.frame(l.page.inset(1, 1, 1, 1), c.light, c.shadow);
    if let Some(&t) = l.tabs.get(p.page) {
        let drawn = Rect::new(t.x - 2, t.y - 2, t.w + 4, t.h + 3);
        tab(fb, c, drawn);
        tab_text(fb, p.page, drawn.y);
        if p.focus == Part::Tab(p.page) && keys {
            // Windows 2000's focused tab: the dots run 1px inside its usual place.
            dialog::focus_dots(fb, t.inset(1, 1, 1, 1), (0, 0), c.text);
        }
    }

    let check = |fb: &mut Framebuffer, part: Part, x: i32, y: i32| {
        let Part::Check(ch) = part else { return };
        let enabled = p.enabled(part);
        check_box(fb, c, x, y, p.is_checked(ch) && enabled, held(part), enabled);
        let label = check_label(ch);
        let tx = x + CHECK_TEXT_X;
        if enabled {
            font.draw_label(fb, (tx, y), label, i32::MAX, c.text, keys);
        } else {
            font.draw_label(fb, (tx + 1, y + 1), label, i32::MAX, c.etch, keys);
            font.draw_label(fb, (tx, y), label, i32::MAX, c.shadow, keys);
        }
        if p.focus == part && keys {
            let w = font.measure_label(label);
            dialog::focus_dots(fb, Rect::new(tx - 1, y - 1, w + 2, font.height + 3), (x, y - 1), c.text);
        }
    };
    let option = |fb: &mut Framebuffer, part: Part, on: bool, x: i32, y: i32| {
        let rows = if on { &OPTION_ON } else { &OPTION_OFF };
        dialog::art(fb, rows, x, y + 2, |b| match b {
            b'g' => Some(c.shadow),
            b'#' => Some(c.dark_shadow),
            b'W' => Some(if held(part) { c.face } else { c.highlight }),
            b'l' => Some(c.light),
            _ => None,
        });
        let label = label_of(era, part).unwrap_or_default();
        let (tx, ty) = (x + 17, y + 2);
        font.draw_label(fb, (tx, ty), &label, i32::MAX, c.text, keys);
        if p.focus == part && keys {
            let w = font.measure_label(&label);
            dialog::focus_dots(fb, Rect::new(tx - 1, ty - 1, w + 2, font.height + 3), (x, y), c.text);
        }
    };
    let framed = p.default_button();
    let button = |fb: &mut Framebuffer, part: Part| {
        if let Some(r) = l.rect(part) {
            let focused = p.focus == part && keys;
            let label = button_label(era, part);
            dialog::push_button(fb, theme, font, r, label, framed == part, held(part), focused, p.enabled(part), keys);
        }
    };
    let lines = |fb: &mut Framebuffer, x: i32, y: i32, text: &[&str]| {
        for (i, line) in text.iter().enumerate() {
            font.draw(fb, x, y + 13 * i as i32, line, c.text);
        }
    };

    match p.current_page() {
        Page::TaskbarOptions => {
            paint_preview(fb, theme, PREVIEW, p.values.options);
            for &(ch, x, y) in option_checks(era) {
                check(fb, Part::Check(ch), x, y);
            }
        }
        Page::StartMenuPrograms if is_2000(era) => {
            group_box(fb, c, font, ADVANCED_GROUP, "Customize Start menu", false);
            W2K_CUSTOMIZE_ICON.paint(fb, 42, 79);
            lines(fb, 85, 79, &["You can customize your", "Start menu by adding or", "removing items from it."]);
            W2K_CLEAR_ICON.paint(fb, 42, 186);
            lines(
                fb,
                87,
                186,
                &["To remove records of", "recently accessed", "documents, programs, and", "Web sites,  click Clear."],
            );
            for (part, _) in ADVANCED_BUTTONS {
                button(fb, part);
            }
            paint_settings_list(fb, c, font, p, keys);
        }
        Page::StartMenuPrograms => {
            group_box(fb, c, font, CUSTOMIZE_GROUP, "Customize Start menu", false);
            IE4_CUSTOMIZE_ICON.paint(fb, 43, 92);
            lines(fb, 91, 92, &["You can customize your Start menu by", "adding or removing items from it."]);
            group_box(fb, c, font, DOCUMENTS_GROUP, "Documents menu", false);
            IE4_DOCUMENTS_ICON.paint(fb, 43, 238);
            lines(
                fb,
                91,
                227,
                &[
                    "Click the Clear button to remove the",
                    "contents of the Documents menu and",
                    "other personal history lists.",
                ],
            );
            for (part, _) in PAGE_BUTTONS {
                button(fb, part);
            }
        }
        Page::Bevel95 => {
            group_box(fb, c, font, LOOK_GROUP, "Look", false);
            for (e, x, y) in LOOK_ROWS {
                option(fb, Part::Look(e), p.values.era == e, x, y);
            }
            group_box(fb, c, font, COLORS_GROUP, "Colors", false);
            for (d, x, y) in COLORS_ROWS {
                option(fb, Part::Colors(d), p.values.colors == d, x, y);
            }
            group_box(fb, c, font, SIZE_GROUP, "Size", false);
            for (list, label_x, field) in [(Part::SizeList, SIZE_LABEL_X, SIZE_FIELD), (Part::WayList, WAY_LABEL_X, WAY_FIELD)] {
                let enabled = p.enabled(list);
                let label = label_of(era, list).unwrap_or_default();
                if enabled {
                    font.draw_label(fb, (label_x, SIZE_TEXT_Y), &label, i32::MAX, c.text, keys);
                } else {
                    font.draw_label(fb, (label_x + 1, SIZE_TEXT_Y + 1), &label, i32::MAX, c.etch, keys);
                    font.draw_label(fb, (label_x, SIZE_TEXT_Y), &label, i32::MAX, c.shadow, keys);
                }
                paint_list_field(fb, theme, font, p, list, field, enabled, keys);
            }
            let line = if p.values.scale.is_whole() { SIZE_WHOLE_TEXT } else { SIZE_QUARTER_TEXT };
            lines(fb, 36, SIZE_LINE_Y, &[line]);
            group_box(fb, c, font, STARTUP_GROUP, "Startup", false);
            check(fb, Part::Check(Check::StartWithWindows), STARTUP_CHECK.0, STARTUP_CHECK.1);
            group_box(fb, c, font, CLOCK_GROUP, "Clock", false);
            let [first, second] = clock_lines(p.values.clock_offset);
            lines(fb, 36, CLOCK_BUTTON_Y - 1, &[&first, &second]);
            button(fb, Part::UsePcTime);
        }
        Page::OwnTaskbar => {
            group_box(fb, c, font, KEY_GROUP, "Windows key", false);
            for (key, x, y) in KEY_ROWS {
                option(fb, Part::WinKey(key), p.values.windows_key == key, x, y);
            }
            group_box(fb, c, font, BUTTONS_GROUP, "Buttons", false);
            check(fb, Part::Check(Check::Group), GROUP_CHECK.0, GROUP_CHECK.1);
            lines(fb, 36, GROUP_CHECK.1 + 22, &[GROUP_TEXT]);
            check(fb, Part::Check(Check::QuickLaunch), QUICK_LAUNCH_CHECK.0, QUICK_LAUNCH_CHECK.1);
            group_box(fb, c, font, SEARCH_GROUP, "Quick Search", false);
            check(fb, Part::Check(Check::QuickSearchAll), SEARCH_CHECK.0, SEARCH_CHECK.1);
            group_box(fb, c, font, BATTERY_GROUP, "Battery", false);
            check(fb, Part::Check(Check::BatteryPercent), BATTERY_CHECK.0, BATTERY_CHECK.1);
        }
        Page::Desktop => {
            group_box(fb, c, font, DESKTOP_GROUP, "Desktop", false);
            check(fb, Part::Check(Check::Desktop), DESKTOP_CHECK.0, DESKTOP_CHECK.1);
            lines(fb, 36, DESKTOP_CHECK.1 + 22, &DESKTOP_TEXT);
        }
        Page::Programs => {
            group_box(fb, c, font, PROGRAMS_GROUP, "The Programs menu shows", false);
            for (shown, x, y) in SHOWN_ROWS {
                option(fb, Part::Shown(shown), p.values.programs == shown, x, y);
            }
            paint_program_list(fb, theme, font, p);
        }
    }
    for (part, _) in BUTTONS {
        button(fb, part);
    }
    // An open drop-down list, over everything (as Shut Down's in Windows 2000: a 1px frame,
    // white rows of 13px, the highlighted one in the menu's highlight).
    if p.current_page() == Page::Bevel95
        && let Some((list, hover)) = p.drop
        && let Some(frame) = drop_frame(font, list)
    {
        fb.fill(frame, rgb(0xFFFFFF));
        fb.frame(frame, c.text, c.text);
        for (i, label) in list_choices(list).iter().enumerate() {
            let row = Rect::new(frame.x + 1, frame.y + 1 + DROP_ROW_H * i as i32, frame.w - 2, DROP_ROW_H);
            let color = if i == hover {
                fb.fill(row, c.menu_highlight);
                c.menu_highlight_text
            } else {
                c.text
            };
            font.draw(fb, row.x + 2, row.y, label, color);
        }
    }
}

/// A drop-down list's field (as Shut Down's in Windows 2000): the sunken field, what is chosen,
/// highlighted while it has the focus and is closed, and the button with its arrow. Not usable:
/// the face color inside and the text grayed (PROVISIONAL: not measured).
#[allow(clippy::too_many_arguments)]
fn paint_list_field(fb: &mut Framebuffer, theme: &Theme, font: &Font, p: &Properties, list: Part, r: Rect, enabled: bool, keys: bool) {
    let c = &theme.colors;
    let field = dialog::well(fb, r, c);
    let item = Rect::new(field.x + 1, field.y + 1, field.w - 2 - 16, field.h - 2);
    let text = p.list_text(list);
    if !enabled {
        fb.fill(field, c.face);
        font.draw(fb, item.x + 2, item.y + 2, &text, c.etch);
        font.draw(fb, item.x + 1, item.y + 1, &text, c.shadow);
    } else {
        let focused = p.focus == list && p.drop.is_none();
        let color = if focused {
            fb.fill(item, c.menu_highlight);
            c.menu_highlight_text
        } else {
            c.text
        };
        font.draw(fb, item.x + 1, item.y + 1, &text, color);
        if focused && keys {
            dialog::invert_dots(fb, theme, item);
        }
    }
    dialog::drop_down(fb, Rect::new(field.right() - 16, field.y, 16, field.h), c, p.pressed == Some(list));
}

/// Windows 2000's Start Menu Settings: the label, a sunken white list of check box rows, and
/// its scroll bar. The selected row is highlighted while the list has the focus.
fn paint_settings_list(fb: &mut Framebuffer, c: &Colors, font: &Font, p: &Properties, keys: bool) {
    font.draw_label(fb, (24, 256), LIST_LABEL, i32::MAX, c.text, keys);
    fb.frame(LIST, c.shadow, c.highlight);
    fb.frame(LIST.inset(1, 1, 1, 1), c.dark_shadow, c.light);
    let inside = list_inside();
    let mut rows = Framebuffer::new(inside.w - 16, inside.h, rgb(0xFFFFFF));
    let focused = p.focus == Part::List;
    for (i, &s) in SETTINGS.iter().enumerate() {
        let r = list_row_rect(i, p.list_top);
        let y = r.y - inside.y;
        if y + LIST_ROW_H <= 0 || y >= inside.h {
            continue;
        }
        let ink = if s.supported() { c.text } else { c.shadow };
        tree::flat_box(&mut rows, 2, y + 2, p.is_on(s), ink, rgb(0xFFFFFF));
        let text_w = font.measure(s.label());
        let mut color = ink;
        if focused && i == p.list_sel {
            // PROVISIONAL: the selection as Windows 2000's list views draw it.
            rows.fill(Rect::new(20, y, text_w + 3, LIST_ROW_H), c.menu_highlight);
            color = c.menu_highlight_text;
            if keys {
                dialog::focus_dots(&mut rows, Rect::new(20, y, text_w + 3, LIST_ROW_H), (20, y), c.menu_highlight_text);
            }
        }
        font.draw(&mut rows, 21, y + 1, s.label(), color);
    }
    fb.blit(&rows, inside.x, inside.y);
    let held = |part: Part| p.pressed == Some(part);
    settings_scroll().paint(fb, c, p.list_top, held(Part::ScrollUp), held(Part::ScrollDown));
}

/// Bevel95 Settings' list of programs: a sunken white tree (`tree::paint_row`: lines, the
/// folders' plus and minus boxes, each row's check box, picture and name) and its scroll bar.
/// The selected row's name is highlighted while the list has the focus.
fn paint_program_list(fb: &mut Framebuffer, theme: &Theme, font: &Font, p: &Properties) {
    let c = &theme.colors;
    fb.frame(PROGRAMS_LIST, c.shadow, c.highlight);
    fb.frame(PROGRAMS_LIST.inset(1, 1, 1, 1), c.dark_shadow, c.light);
    let (inside, list, w) = (programs_inside(), &p.programs, program_rows_width(p));
    let mut rows = Framebuffer::new(w, inside.h, rgb(0xFFFFFF));
    let (style, focused) = (TreeStyle::of(p.era), p.focus == Part::Programs);
    let folder = crate::icons::tree_group(p.era);
    if !list.loaded {
        font.draw(&mut rows, 4, 2, PROGRAMS_WAIT, c.shadow);
    }
    let screen = (p.screen.0 + inside.x, p.screen.1 + inside.y);
    for (at, &i) in p.program_rows().iter().enumerate().skip(list.top).take(PROGRAM_PAGE) {
        let row = &list.rows[i];
        let tree_row = TreeRow {
            depth: row.depth,
            open: (row.kind != RowKind::Item).then_some(row.open),
            last: programs::lasts(&list.rows, i),
            tick: Some(p.row_tick(i)),
            // "Programs" and its folders have the picture they have in the Start menu's
            // cascades and in Windows' own tree of them.
            icon: if row.kind == RowKind::Item { row.icon.as_ref() } else { Some(&folder) },
            label: &row.name,
            selected: focused && i == list.sel,
            focused,
        };
        tree::paint_row(&mut rows, theme, font, style, &tree_row, (at - list.top) as i32 * tree::ROW_H, w, screen);
    }
    fb.blit(&rows, inside.x, inside.y);
    if let Some(scroll) = programs_scroll(p) {
        let held = |part: Part| p.pressed == Some(part);
        scroll.paint(fb, c, list.top, held(Part::ScrollUp), held(Part::ScrollDown));
    }
}

/// Taskbar Properties or Bevel95 Settings as a window of the control kit (`kit::Form`).
#[derive(Clone, Debug)]
pub struct PropertiesWindow {
    pub props: Properties,
    pub layout: PropertiesLayout,
}

impl PropertiesWindow {
    pub fn new(props: Properties, fonts: &Fonts) -> PropertiesWindow {
        let layout = layout(&props, fonts);
        PropertiesWindow { props, layout }
    }

    /// What of row `i` of the list of programs is at `x` (in the window). Everything right of
    /// the check box counts as the row's picture and name: a click there selects the row (in
    /// Windows' trees only a click on the name itself does; here the names are short and the
    /// rows many).
    fn program_zone(&self, i: usize, x: i32) -> Zone {
        let (Some(r), Some(row)) = (self.layout.rect(Part::ProgramRow(i)), self.props.programs.rows.get(i)) else {
            return Zone::Blank;
        };
        tree::zone(TreeStyle::of(self.props.era), row.depth, row.kind != RowKind::Item, true, r.w, x - r.x)
    }

    /// Ctrl+Tab and the like: the next or the previous page (the focus on the tabs stays there).
    fn page_step(&mut self, forward: bool) {
        let p = &mut self.props;
        let n = p.sheet.pages().len();
        let on_tabs = matches!(p.focus, Part::Tab(_));
        let next = if forward { (p.page + 1) % n } else { (p.page + n - 1) % n };
        p.show_page(next);
        if on_tabs {
            p.focus = Part::Tab(next);
        }
    }
}

impl kit::Form for PropertiesWindow {
    type Part = Part;

    fn title(&self) -> &str {
        self.layout.title
    }
    fn size(&self) -> (i32, i32) {
        (self.layout.width, self.layout.height)
    }
    fn layout(&mut self, fonts: &Fonts) {
        self.layout = layout(&self.props, fonts);
    }
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        paint(fb, theme, fonts, &self.props, &self.layout);
    }
    fn set_screen(&mut self, x: i32, y: i32) {
        self.props.screen = (x, y);
    }
    /// The second press of a double-click on a folder's picture or name in the list of
    /// programs: its rows show or are put away, as in Windows' trees. Anywhere else it is one
    /// more press.
    fn double_click(&mut self, x: i32, y: i32) -> Option<Vec<Effect<Part>>> {
        let Some(Part::ProgramRow(i)) = self.layout.hit(x, y) else { return None };
        if self.program_zone(i, x) != Zone::Item {
            return None;
        }
        let p = &mut self.props;
        let open = p.programs.rows.get(i).filter(|row| row.kind != RowKind::Item)?.open;
        (p.focus, p.programs.sel) = (Part::Programs, i);
        p.open_program(i, !open);
        Some(Vec::new())
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.layout.hit(x, y)
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        self.layout.rect(part)
    }
    fn enabled(&self, part: Part) -> bool {
        self.props.enabled(part)
    }
    fn focus(&self) -> Part {
        self.props.focus
    }
    fn set_focus(&mut self, part: Part) {
        self.props.focus = part;
        self.props.drop = None;
    }
    fn tab_stops(&self) -> Vec<Part> {
        self.props.tab_stops()
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.props.pressed = part;
    }
    fn show_keys(&mut self) {
        self.props.show_keys = true;
    }
    fn enter(&self) -> Part {
        self.props.default_button()
    }
    fn escape(&self) -> Part {
        Part::Cancel
    }

    fn key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        let p = &mut self.props;
        let (list, programs) = (p.focus == Part::List, p.focus == Part::Programs);
        let options = matches!(p.focus, Part::Look(_) | Part::Colors(_) | Part::WinKey(_) | Part::Shown(_));
        let drop_list = matches!(p.focus, Part::SizeList | Part::WayList).then_some(p.focus);
        match key {
            // A drop-down list: Enter takes the highlighted entry and Esc leaves the choice as it
            // was (both only close the open list), F4 opens and closes it, the arrows step.
            Key::Enter if p.drop.is_some() => {
                if let Some((list, hover)) = p.drop.take() {
                    p.choose_entry(list, hover);
                }
            }
            Key::Escape if p.drop.is_some() => p.drop = None,
            Key::F4 if drop_list.is_some() => p.toggle_list(p.focus),
            Key::Up | Key::Down | Key::Left | Key::Right if drop_list.is_some() => {
                let open = p.drop.is_some();
                if !open || matches!(key, Key::Up | Key::Down) {
                    p.step_list(p.focus, matches!(key, Key::Down | Key::Right));
                }
            }
            Key::Home | Key::End if drop_list.is_some() => {
                let n = list_choices(p.focus).len();
                let at = if key == Key::End { n.saturating_sub(1) } else { 0 };
                match p.drop.as_mut() {
                    Some((_, hover)) => *hover = at,
                    None => {
                        p.choose_entry(p.focus, at);
                    }
                }
            }
            Key::Tab if mods.ctrl => self.page_step(!mods.shift),
            Key::PageUp | Key::PageDown if mods.ctrl => self.page_step(key == Key::PageDown),
            Key::Left | Key::Right if matches!(p.focus, Part::Tab(_)) => self.page_step(key == Key::Right),
            Key::Up | Key::Down | Key::Left | Key::Right if options => {
                if let Some(next) = p.arrow(p.focus, matches!(key, Key::Down | Key::Right)) {
                    p.focus = next;
                    p.choose(next);
                }
            }
            Key::Up | Key::Down if list => p.move_selection(if key == Key::Down { 1 } else { -1 }),
            Key::PageUp | Key::PageDown if list => p.move_selection(if key == Key::PageDown { 4 } else { -4 }),
            Key::Home | Key::End if list => {
                let all = SETTINGS.len() as i32;
                p.move_selection(if key == Key::End { all } else { -all });
            }
            Key::Up | Key::Down if programs => p.move_program(if key == Key::Down { 1 } else { -1 }),
            Key::Left | Key::Right if programs => p.step_program(key == Key::Right),
            Key::PageUp | Key::PageDown if programs => {
                let page = PROGRAM_PAGE as i32 - 1;
                p.move_program(if key == Key::PageDown { page } else { -page });
            }
            Key::Home | Key::End if programs => {
                let all = p.programs.rows.len() as i32;
                p.move_program(if key == Key::End { all } else { -all });
            }
            _ => return None,
        }
        Some(Vec::new())
    }
    /// No text fields here: a letter is an access key, with or without Alt. In the list of
    /// programs it goes to the next program of that letter instead, as in Windows' lists.
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        let p = &mut self.props;
        if p.focus == Part::Programs {
            // "+" opens the selected folder, "-" closes it, "*" opens it with every folder in
            // it, as in Windows' trees.
            match ch {
                '+' => p.open_program(p.programs.sel, true),
                '-' => p.open_program(p.programs.sel, false),
                '*' => p.open_all_programs(),
                letter if letter.is_alphanumeric() => {
                    p.show_keys = true;
                    p.find_program(letter);
                }
                _ => {}
            }
            return Vec::new();
        }
        if !ch.is_alphanumeric() {
            return Vec::new();
        }
        p.show_keys = true;
        // In a drop-down list a letter or a digit goes to the next entry that starts with it.
        if matches!(p.focus, Part::SizeList | Part::WayList) {
            let list = p.focus;
            let choices = list_choices(list);
            let from = p.drop.map(|(_, h)| h).or_else(|| p.list_chosen(list)).unwrap_or(0);
            let starts = |i: usize| choices[i].chars().next().is_some_and(|c| c.eq_ignore_ascii_case(&ch));
            if let Some(i) = (1..=choices.len()).map(|k| (from + k) % choices.len()).find(|&i| starts(i)) {
                match p.drop.as_mut() {
                    Some((_, hover)) => *hover = i,
                    None => {
                        p.choose_entry(list, i);
                    }
                }
                return Vec::new();
            }
        }
        self.access(ch)
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        let p = &mut self.props;
        // An open drop-down list closes first (as it was: nothing taken from it), so that
        // what the key reaches is not hidden under it and Enter and Esc mean OK and Cancel.
        p.drop = None;
        match p.access_key(ch) {
            Some(part @ (Part::Check(_) | Part::Look(_) | Part::Colors(_) | Part::WinKey(_) | Part::Shown(_))) => {
                p.focus = part;
                p.choose(part);
            }
            Some(part @ (Part::List | Part::SizeList | Part::WayList)) => {
                p.focus = part;
            }
            Some(part) => {
                p.focus = part;
                return kit::activate(self, part);
            }
            None => {}
        }
        Vec::new()
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        let p = &mut self.props;
        match p.focus {
            Part::List => {
                p.choose(Part::ListRow(p.list_sel));
            }
            Part::Programs => {
                p.choose(Part::ProgramRow(p.programs.sel));
            }
            f @ (Part::Check(_) | Part::Look(_) | Part::Colors(_) | Part::WinKey(_) | Part::Shown(_)) => {
                p.choose(f);
            }
            f if f.is_push_button() => return kit::activate(self, f),
            _ => {}
        }
        Vec::new()
    }
    fn press(&mut self, part: Option<Part>, x: i32, _y: i32) -> Press<Part> {
        let zone = match part {
            Some(Part::ProgramRow(i)) => self.program_zone(i, x),
            _ => Zone::Blank,
        };
        let p = &mut self.props;
        // While a drop-down list is open a press takes its entry, or closes it (on its field
        // or anywhere else; the press does nothing more).
        if let Some((list, _)) = p.drop.take() {
            if let Some(Part::DropRow(i)) = part {
                p.choose_entry(list, i);
            }
            return Press::Done;
        }
        match part {
            Some(list @ (Part::SizeList | Part::WayList)) if p.enabled(list) => {
                p.focus = list;
                p.toggle_list(list);
            }
            Some(Part::Tab(i)) => {
                p.show_page(i);
                p.focus = Part::Tab(i);
            }
            Some(Part::ListRow(i)) => {
                // A click on a row's check box flips it; elsewhere on the row only selects it.
                let on_box = self.layout.rect(Part::ListRow(i)).is_some_and(|r| x < r.x + 18);
                p.focus = Part::List;
                p.list_sel = i;
                if on_box {
                    p.choose(Part::ListRow(i));
                }
            }
            Some(Part::ProgramRow(i)) => {
                // As in Windows' trees: a folder's plus/minus box shows its rows or puts them
                // away, a row's check box flips, its picture or name selects it.
                p.focus = Part::Programs;
                match zone {
                    Zone::Button => {
                        let open = p.programs.rows.get(i).is_some_and(|row| row.open);
                        p.open_program(i, !open);
                    }
                    Zone::Tick => {
                        p.programs.sel = i;
                        p.choose(Part::ProgramRow(i));
                    }
                    Zone::Item => p.programs.sel = i,
                    Zone::Blank => {}
                }
            }
            Some(part @ (Part::ScrollUp | Part::ScrollDown)) => {
                p.scroll(if part == Part::ScrollUp { -1 } else { 1 });
                return Press::Hold(part);
            }
            Some(part @ (Part::ScrollPageUp | Part::ScrollPageDown)) => {
                let page = p.list_size().1 as i32;
                p.scroll(if part == Part::ScrollPageUp { -page } else { page });
            }
            Some(Part::ScrollThumb) => return Press::Grab(Part::ScrollThumb, p.list_top() as i32),
            Some(Part::List) => p.focus = Part::List,
            Some(Part::Programs) => p.focus = Part::Programs,
            Some(part) if p.enabled(part) => {
                if !matches!(part, Part::TitleHelp | Part::TitleClose) {
                    p.focus = part;
                }
                return Press::Hold(part);
            }
            Some(_) => {}
            None => return Press::Nothing,
        }
        Press::Done
    }
    fn release(&mut self, part: Part) -> Vec<Effect<Part>> {
        match part {
            Part::Check(_) | Part::Look(_) | Part::Colors(_) | Part::WinKey(_) | Part::Shown(_) => {
                self.props.choose(part);
                Vec::new()
            }
            p if p.is_push_button() || matches!(p, Part::TitleHelp | Part::TitleClose) => vec![Effect::Activate(p)],
            _ => Vec::new(),
        }
    }
    /// Dragging the thumb: the rows follow, one row per (track - thumb) / (rows - page).
    fn drag(&mut self, _part: Part, grab_y: i32, grab_top: i32, _x: i32, y: i32) -> bool {
        let Some(scroll) = self.props.scroll_bar() else { return false };
        let top = scroll.dragged(grab_top, y - grab_y);
        let changed = top != self.props.list_top();
        self.props.set_list_top(top);
        changed
    }
    /// The wheel turns the list of the page shown: Start Menu Settings by a row for each notch,
    /// the long list of programs by three (Windows' usual).
    /// Alt+Down and Alt+Up open a drop-down list, and close it taking the highlighted entry.
    fn alt_key(&mut self, key: Key) -> bool {
        let p = &mut self.props;
        if !matches!(p.focus, Part::SizeList | Part::WayList) || !matches!(key, Key::Up | Key::Down) {
            return false;
        }
        match p.drop.take() {
            Some((list, hover)) => {
                p.choose_entry(list, hover);
            }
            None => p.toggle_list(p.focus),
        }
        true
    }
    /// The pointer over an open drop-down list highlights the entry under it.
    fn hover(&mut self, x: i32, y: i32) -> bool {
        let Some((list, hover)) = self.props.drop else { return false };
        match self.layout.hit(x, y) {
            Some(Part::DropRow(i)) if i != hover => {
                self.props.drop = Some((list, i));
                true
            }
            _ => false,
        }
    }
    /// Another window came to the front: an open drop-down list closes, as it is.
    fn set_active(&mut self, active: bool) {
        if !active {
            self.props.drop = None;
        }
    }
    fn wheel(&mut self, notches: i32) {
        match self.props.current_page() {
            Page::StartMenuPrograms => self.props.scroll(-notches),
            Page::Programs => self.props.scroll(-3 * notches),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> Values {
        Values {
            options: TaskbarOptions::default(),
            start: StartOptions::default(),
            era: Era::Win95,
            colors: Depth::TrueColor,
            scale: Scale::whole(2),
            start_with_windows: false,
            clock_offset: 0,
            desktop: true,
            programs: Shown::Everything,
            windows_key: WindowsKey::Find,
            group: true,
            quick_search_all: false,
            battery_percent: false,
            quick_launch: false,
        }
    }

    #[test]
    fn use_the_pcs_time() {
        let mut p = Properties::new(Sheet::Bevel95, Era::Win95, values());
        assert!(!p.enabled(Part::UsePcTime) && !p.use_pc_time(), "nothing to undo");
        p.values.clock_offset = 3_594;
        assert_eq!(p.tab_stops()[5], Part::UsePcTime);
        assert_eq!(p.access_key('u'), Some(Part::UsePcTime));
        assert!(p.use_pc_time() && p.values.clock_offset == 0 && p.enabled(Part::Apply));
        assert!(!p.tab_stops().contains(&Part::UsePcTime), "grayed out once done");
        assert_eq!(clock_lines(0)[1], "the PC's time.");
        assert_eq!(clock_lines(3_594)[1], "about 1 hour ahead of the PC.");
        assert_eq!(clock_lines(-300)[1], "5 minutes behind the PC.");
        assert_eq!(amount(1), "1 second");
        assert_eq!(amount(287), "about 5 minutes");
        assert_eq!(amount(86_399), "about 1 day");
        assert_eq!(amount(2 * 86_400), "2 days");
        assert_eq!(amount(100 * 365 * 86_400 + 5), "about 100 years");
    }

    #[test]
    fn opens_on_taskbar_options_with_apply_off() {
        let p = Properties::new(Sheet::Taskbar, Era::Win98, values());
        assert_eq!(p.current_page(), Page::TaskbarOptions);
        assert_eq!(p.focus, Part::Check(Check::AlwaysOnTop));
        assert!(!p.enabled(Part::Apply));
        assert_eq!(p.default_button(), Part::Ok);
        assert!(p.is_checked(Check::AlwaysOnTop) && !p.is_checked(Check::AutoHide));
    }

    #[test]
    fn any_change_turns_apply_on() {
        let mut p = Properties::new(Sheet::Taskbar, Era::Win95, values());
        assert!(p.choose(Part::Check(Check::AutoHide)));
        assert!(p.values.options.auto_hide && p.enabled(Part::Apply));
        assert!(p.choose(Part::Check(Check::AutoHide)));
        assert!(!p.values.options.auto_hide && p.enabled(Part::Apply), "Windows keeps Apply on once something changed");
        assert!(!p.choose(Part::Ok));
        assert!(p.choose_entry(Part::SizeList, 12) && p.values.scale == Scale::whole(4));
    }

    #[test]
    fn tab_goes_through_the_page_then_the_buttons() {
        let p = Properties::new(Sheet::Taskbar, Era::Win95, values());
        let stops = p.tab_stops();
        assert_eq!(stops[0], Part::Tab(0));
        assert_eq!(
            stops[1..5],
            [Check::AlwaysOnTop, Check::AutoHide, Check::SmallIcons, Check::ShowClock].map(Part::Check)
        );
        assert_eq!(stops[5..], [Part::Ok, Part::Cancel], "Apply is skipped while grayed out");
        let p = Properties::new(Sheet::Bevel95, Era::Win95, values());
        let stops = p.tab_stops();
        assert_eq!(stops[0], Part::Tab(0));
        assert_eq!(
            stops[1..],
            [
                Part::Look(Era::Win95),
                Part::Colors(Depth::TrueColor),
                Part::SizeList,
                Part::Check(Check::StartWithWindows),
                Part::Ok,
                Part::Cancel
            ]
        );
        assert_eq!(p.focus, Part::Look(Era::Win95), "it opens with the focus on the look in use");
    }

    #[test]
    fn the_windows_key_group_chooses_find_or_the_start_menu() {
        let mut p = Properties::new(Sheet::Bevel95, Era::Win98, values());
        assert_eq!(p.access_key('m'), None, "it is on the Taskbar page");
        p.show_page(1);
        assert_eq!((p.current_page(), p.focus), (Page::OwnTaskbar, Part::WinKey(WindowsKey::Find)));
        assert_eq!(p.access_key('m'), Some(Part::WinKey(WindowsKey::Start)));
        assert_eq!(p.access_key('q'), Some(Part::WinKey(WindowsKey::Find)));
        assert_eq!(p.arrow(Part::WinKey(WindowsKey::Find), true), Some(Part::WinKey(WindowsKey::Start)));
        assert!(p.choose(Part::WinKey(WindowsKey::Start)));
        assert_eq!(p.values.windows_key, WindowsKey::Start);
        assert!(p.enabled(Part::Apply));
        assert!(p.tab_stops().contains(&Part::WinKey(WindowsKey::Start)), "the chosen button is the group's stop");
        // Quick Search has one name in every look.
        let mut p = Properties::new(Sheet::Bevel95, Era::Win2000, values());
        p.show_page(1);
        for era in Era::ALL {
            assert_eq!(label_of(era, Part::WinKey(WindowsKey::Find)).as_deref(), Some("Opens &Quick Search"));
        }
        assert_eq!(p.access_key('q'), Some(Part::WinKey(WindowsKey::Find)));
        // Bevel95 Settings puts it into effect; Taskbar Properties leaves it as it is.
        let now = Values { windows_key: WindowsKey::Start, ..values() };
        assert_eq!(Sheet::Bevel95.merge(now, values()).windows_key, WindowsKey::Start);
        assert_eq!(Sheet::Taskbar.merge(values(), now).windows_key, WindowsKey::Start);
    }

    #[test]
    fn the_taskbar_page_switches_grouping() {
        let mut p = Properties::new(Sheet::Bevel95, Era::Win95, values());
        p.show_page(1);
        assert_eq!(page_title(Era::Win2000, Page::OwnTaskbar), "Taskbar");
        let stops = [
            Part::Tab(1),
            Part::WinKey(WindowsKey::Find),
            Part::Check(Check::Group),
            Part::Check(Check::QuickLaunch),
            Part::Check(Check::QuickSearchAll),
            Part::Check(Check::BatteryPercent),
            Part::Ok,
            Part::Cancel,
        ];
        assert_eq!(p.tab_stops(), stops);
        assert_eq!(p.access_key('g'), Some(Part::Check(Check::Group)));
        // Quick Launch (PROVISIONAL words): shows what holds, Bevel95 Settings puts it into effect.
        assert_eq!(p.access_key('l'), Some(Part::Check(Check::QuickLaunch)));
        assert!(!p.is_checked(Check::QuickLaunch));
        let on = Values { quick_launch: true, ..values() };
        assert!(Sheet::Bevel95.merge(on, values()).quick_launch);
        assert!(!Sheet::Taskbar.merge(on, values()).quick_launch, "Taskbar Properties leaves it as in effect");
        assert!(p.is_checked(Check::Group), "grouped, as the bar always was");
        assert!(p.choose(Part::Check(Check::Group)));
        assert!(!p.values.group && p.enabled(Part::Apply));
        // Bevel95's own: Windows' window leaves it as it is in effect.
        let now = Values { group: false, ..values() };
        assert!(!Sheet::Taskbar.merge(values(), now).group);
        assert!(Sheet::Bevel95.merge(values(), now).group);
    }

    #[test]
    fn the_taskbar_page_says_what_quick_search_lists() {
        let mut p = Properties::new(Sheet::Bevel95, Era::Win98, values());
        p.show_page(1);
        assert!(!p.is_checked(Check::QuickSearchAll), "programs only by default");
        assert_eq!(p.access_key('d'), Some(Part::Check(Check::QuickSearchAll)));
        assert!(p.choose(Part::Check(Check::QuickSearchAll)));
        assert!(p.values.quick_search_all && p.enabled(Part::Apply));
        let now = Values { quick_search_all: true, ..values() };
        assert!(Sheet::Taskbar.merge(values(), now).quick_search_all);
        assert!(!Sheet::Bevel95.merge(values(), now).quick_search_all);
    }

    #[test]
    fn the_taskbar_page_shows_the_battery_percentage() {
        for era in Era::ALL {
            let mut p = Properties::new(Sheet::Bevel95, era, values());
            p.show_page(1);
            assert!(!p.is_checked(Check::BatteryPercent), "off unless switched on");
            assert_eq!(p.access_key('b'), Some(Part::Check(Check::BatteryPercent)));
            assert!(p.choose(Part::Check(Check::BatteryPercent)));
            assert!(p.values.battery_percent && p.enabled(Part::Apply));
        }
        // Bevel95's own: Bevel95 Settings puts it into effect, Taskbar Properties leaves it.
        let now = Values { battery_percent: true, ..values() };
        assert!(Sheet::Taskbar.merge(values(), now).battery_percent);
        assert!(!Sheet::Bevel95.merge(values(), now).battery_percent);
        assert!(Sheet::Bevel95.merge(now, values()).battery_percent);
    }

    #[test]
    fn windows_2000_has_its_own_pages() {
        let mut p = Properties::new(Sheet::Taskbar, Era::Win2000, values());
        assert_eq!(title(Sheet::Taskbar, Era::Win2000), "Taskbar and Start Menu Properties");
        assert_eq!(page_title(Era::Win2000, Page::StartMenuPrograms), "Advanced");
        // Personalized Menus is shown but grayed out, so Tab skips it.
        assert!(!p.tab_stops().contains(&Part::Check(Check::PersonalizedMenus)));
        assert!(!p.choose(Part::Check(Check::PersonalizedMenus)));
        p.show_page(1);
        assert_eq!(p.focus, Part::Add);
        assert_eq!(p.access_key('d'), Some(Part::Add), "Windows 2000 says A&dd...");
        assert_eq!(p.access_key('v'), Some(Part::Advanced));
        assert_eq!(p.access_key('t'), Some(Part::List));
        assert!(p.tab_stops().contains(&Part::List));
    }

    #[test]
    fn start_menu_settings_list() {
        let mut p = Properties::new(Sheet::Taskbar, Era::Win2000, values());
        assert!(p.is_on(Setting::LogOff) && !p.is_on(Setting::Favorites));
        assert!(p.choose(Part::ListRow(1)) && p.values.start.favorites);
        assert!(!p.choose(Part::ListRow(0)), "Administrative Tools is not done");
        p.move_selection(6);
        assert_eq!((p.list_sel, p.list_top), (6, 3));
        p.scroll(-10);
        assert_eq!(p.list_top, 0);
        p.scroll(10);
        assert_eq!(p.list_top, 4);
        // The thumb: 37px of track, half of it for 4 of 8 rows (19 after rounding), at the top.
        assert_eq!(thumb_rect(0), Rect::new(303, 290, 16, 19));
        assert_eq!(thumb_rect(4).bottom(), 327);
    }

    #[test]
    fn the_size_lists_choose_every_size_and_how_it_is_drawn() {
        let font = one_glyph_font();
        let fonts = Fonts { regular: &font, bold: &font };
        let mut w = PropertiesWindow::new(Properties::new(Sheet::Bevel95, Era::Win98, values()), &fonts);
        // Every size the menus offer, 1 to 4 in quarter steps; the size in use chosen.
        assert_eq!(list_choices(Part::SizeList).len(), MENU_SCALES.len());
        assert_eq!(w.props.list_chosen(Part::SizeList), MENU_SCALES.iter().position(|s| s.quarters() == 8));
        assert!(!w.props.enabled(Part::WayList), "at a whole size the way does not matter");
        // The keyboard: the arrows step the closed list at once.
        w.props.focus = Part::SizeList;
        assert!(kit::Form::key(&mut w, Key::Down, Mods::default()).is_some());
        assert_eq!(w.props.values.scale.quarters(), 9);
        assert!(w.props.changed && w.props.enabled(Part::WayList));
        // F4 opens it on the chosen entry; Down moves the highlight only; Enter takes it.
        kit::Form::key(&mut w, Key::F4, Mods::default());
        assert_eq!(w.props.drop, Some((Part::SizeList, 5)));
        kit::Form::key(&mut w, Key::Down, Mods::default());
        assert_eq!((w.props.drop, w.props.values.scale.quarters()), (Some((Part::SizeList, 6)), 9));
        kit::Form::key(&mut w, Key::Enter, Mods::default());
        assert_eq!((w.props.drop, w.props.values.scale.quarters()), (None, 10));
        // Esc closes it as it was.
        kit::Form::key(&mut w, Key::F4, Mods::default());
        kit::Form::key(&mut w, Key::Up, Mods::default());
        kit::Form::key(&mut w, Key::Escape, Mods::default());
        assert_eq!((w.props.drop, w.props.values.scale.quarters()), (None, 10));
        // The mouse: a press on the field opens it, its rows lie over what is under them, a press
        // on one takes it.
        kit::Form::layout(&mut w, &fonts);
        let field = w.layout.rect(Part::WayList).unwrap();
        kit::Form::press(&mut w, Some(Part::WayList), field.x + 4, field.y + 4);
        kit::Form::layout(&mut w, &fonts);
        assert_eq!(w.props.drop, Some((Part::WayList, 0)));
        let row = w.layout.rect(Part::DropRow(3)).unwrap();
        assert_eq!(kit::Form::hit(&w, row.x + 2, row.y + 2), Some(Part::DropRow(3)));
        kit::Form::press(&mut w, Some(Part::DropRow(3)), row.x + 2, row.y + 2);
        assert_eq!((w.props.drop, w.props.values.scale.way, w.props.values.scale.quarters()), (None, Way::Smooth, 10));
        // A digit goes to the next size that starts with it.
        w.props.focus = Part::SizeList;
        kit::Form::typed(&mut w, '3');
        assert_eq!(w.props.values.scale.quarters(), 12);
        // Back at a whole size the way stays as chosen, grayed.
        assert!(!w.props.enabled(Part::WayList) && w.props.values.scale.way == Way::Smooth);
    }

    #[test]
    fn a_key_for_another_control_closes_the_open_list_first() {
        let font = one_glyph_font();
        let fonts = Fonts { regular: &font, bold: &font };
        let mut w = PropertiesWindow::new(Properties::new(Sheet::Bevel95, Era::Win95, values()), &fonts);
        let size = w.props.values.scale;
        // A letter or digit no size begins with ("8" is Windows 98's access key).
        w.props.focus = Part::SizeList;
        kit::Form::key(&mut w, Key::F4, Mods::default());
        assert!(w.props.drop.is_some());
        kit::Form::typed(&mut w, '8');
        assert_eq!((w.props.drop, w.props.focus, w.props.values.scale), (None, Part::Look(Era::Win98), size));
        // Enter is the window's again (OK), not the list's.
        assert!(kit::Form::key(&mut w, Key::Enter, Mods::default()).is_none());
        // Alt and a letter.
        w.props.focus = Part::SizeList;
        kit::Form::key(&mut w, Key::F4, Mods::default());
        kit::Form::access(&mut w, '9');
        assert_eq!((w.props.drop, w.props.focus, w.props.values.scale), (None, Part::Look(Era::Win95), size));
        assert!(kit::Form::key(&mut w, Key::Escape, Mods::default()).is_none(), "Esc is Cancel again");
    }

    #[test]
    fn a_size_the_list_does_not_offer_steps_to_the_nearest_it_does() {
        let font = one_glyph_font();
        let fonts = Fonts { regular: &font, bold: &font };
        let largest = MENU_SCALES.len() - 1;
        for (forward, key) in [(true, Key::Down), (false, Key::Up)] {
            let mut v = values();
            v.scale = Scale::whole(5);
            let mut w = PropertiesWindow::new(Properties::new(Sheet::Bevel95, Era::Win98, v), &fonts);
            assert_eq!(w.props.list_chosen(Part::SizeList), None);
            w.props.focus = Part::SizeList;
            kit::Form::key(&mut w, key, Mods::default());
            assert_eq!(w.props.values.scale, MENU_SCALES[largest], "forward {forward}");
        }
        // Opened, the list shows the nearest size highlighted.
        let mut v = values();
        v.scale = Scale::from_quarters(17);
        let mut w = PropertiesWindow::new(Properties::new(Sheet::Bevel95, Era::Win98, v), &fonts);
        w.props.focus = Part::SizeList;
        kit::Form::key(&mut w, Key::F4, Mods::default());
        assert_eq!(w.props.drop, Some((Part::SizeList, largest)));
    }

    #[test]
    fn access_keys_and_arrows() {
        let mut p = Properties::new(Sheet::Taskbar, Era::Win95, values());
        assert_eq!(p.access_key('U'), Some(Part::Check(Check::AutoHide)));
        assert_eq!(p.access_key('c'), Some(Part::Check(Check::ShowClock)));
        assert_eq!(p.access_key('a'), None, "Apply is grayed out");
        p.show_page(1);
        assert_eq!(p.access_key('d'), Some(Part::Advanced));
        let p = Properties::new(Sheet::Bevel95, Era::Win95, values());
        assert_eq!(p.access_key('8'), Some(Part::Look(Era::Win98)));
        assert_eq!(p.access_key('z'), Some(Part::SizeList));
        assert_eq!(p.access_key('d'), None, "how a size is drawn is grayed at a whole size");
        assert_eq!(p.arrow(Part::Look(Era::Win95), false), Some(Part::Look(Era::Win95)));
        // The colors: C, 6 and T, the arrows between the three.
        assert_eq!(p.access_key('c'), Some(Part::Colors(Depth::Sixteen)));
        assert_eq!(p.access_key('6'), Some(Part::Colors(Depth::TwoFiftySix)));
        assert_eq!(p.access_key('t'), Some(Part::Colors(Depth::TrueColor)));
        assert_eq!(p.arrow(Part::Colors(Depth::Sixteen), true), Some(Part::Colors(Depth::TwoFiftySix)));
        assert_eq!(p.arrow(Part::Colors(Depth::TwoFiftySix), true), Some(Part::Colors(Depth::TrueColor)));
        let mut p = p;
        assert!(p.choose(Part::Colors(Depth::Sixteen)) && p.values.colors == Depth::Sixteen && p.changed);
        assert!(p.choose(Part::Colors(Depth::TwoFiftySix)) && p.values.colors == Depth::TwoFiftySix);
    }

    #[test]
    fn each_window_has_its_own_pages_and_settings() {
        assert_eq!(Sheet::Taskbar.pages(), [Page::TaskbarOptions, Page::StartMenuPrograms]);
        assert_eq!(Sheet::Bevel95.pages(), [Page::Bevel95, Page::OwnTaskbar, Page::Desktop, Page::Programs]);
        assert_eq!(title(Sheet::Bevel95, Era::Win2000), "Bevel95 Settings");
        assert_eq!(page_title(Era::Win95, Page::Bevel95), "General");
        assert_eq!(page_title(Era::Win2000, Page::Desktop), "Desktop");
        // Windows' window was opened first; meanwhile Bevel95 Settings changed the look, the
        // size, starting with Windows and the clock. Its Apply must not put the old ones back.
        let opened = values();
        let now = Values { era: Era::Win2000, scale: Scale::whole(3), start_with_windows: true, clock_offset: 60, ..opened };
        let mut taskbar = Properties::new(Sheet::Taskbar, Era::Win95, opened);
        assert!(taskbar.choose(Part::Check(Check::AutoHide)));
        let applied = Sheet::Taskbar.merge(taskbar.values, now);
        assert!(applied.options.auto_hide, "its own choice");
        assert_eq!((applied.era, applied.scale), (Era::Win2000, Scale::whole(3)));
        assert_eq!((applied.start_with_windows, applied.clock_offset), (true, 60));
        // The other way round: Bevel95 Settings leaves the taskbar's options as they are now.
        let now = Values { options: TaskbarOptions { show_clock: false, ..opened.options }, ..opened };
        let mut settings = Properties::new(Sheet::Bevel95, Era::Win95, opened);
        assert!(settings.choose(Part::Look(Era::Win98)));
        let applied = Sheet::Bevel95.merge(settings.values, now);
        assert_eq!((applied.era, applied.options.show_clock), (Era::Win98, false));
    }

    #[test]
    fn the_desktop_page_switches_the_desktop() {
        let mut p = Properties::new(Sheet::Bevel95, Era::Win2000, values());
        p.show_page(2);
        assert_eq!(p.current_page(), Page::Desktop);
        assert_eq!(p.focus, Part::Check(Check::Desktop), "its one control");
        assert_eq!(p.tab_stops(), [Part::Tab(2), Part::Check(Check::Desktop), Part::Ok, Part::Cancel]);
        assert_eq!(p.access_key('s'), Some(Part::Check(Check::Desktop)));
        assert!(p.is_checked(Check::Desktop));
        assert!(p.choose(Part::Check(Check::Desktop)));
        assert!(!p.values.desktop && p.enabled(Part::Apply));
        // The desktop is Bevel95's own: Windows' window leaves it as it is in effect.
        let now = Values { desktop: false, ..values() };
        assert!(!Sheet::Taskbar.merge(values(), now).desktop);
        assert!(Sheet::Bevel95.merge(values(), now).desktop);
    }

    /// A list of programs: "Programs", a folder with `in_folder` programs (the first two
    /// Bevel95's own), then `beside` programs in Programs itself. Every program ticked.
    fn program_rows(in_folder: usize, beside: usize) -> Vec<Row> {
        let mut tree = crate::programs::Folder::default();
        let item = |name: String| crate::programs::Item { target: format!("C:\\{name}.lnk"), name };
        for i in 0..in_folder {
            tree.add(&["Games"], item(format!("Game {i:02}")));
        }
        for i in 0..beside {
            tree.add(&[], item(format!("{} {i:02}", if i % 2 == 0 { "Paint" } else { "Notepad" })));
        }
        tree.sort();
        programs::rows(&tree, None, |item| (matches!(item.name.as_str(), "Game 00" | "Game 01"), None))
    }

    fn ticked(p: &Properties) -> Vec<&str> {
        programs::ticked_places(&p.programs.rows).collect()
    }

    #[test]
    fn the_programs_page_chooses_what_the_menu_shows() {
        let mut p = Properties::new(Sheet::Bevel95, Era::Win98, values());
        p.show_page(3);
        assert_eq!((p.current_page(), p.focus), (Page::Programs, Part::Shown(Shown::Everything)));
        let stops = [Part::Tab(3), Part::Shown(Shown::Everything), Part::Programs, Part::Ok, Part::Cancel];
        assert_eq!(p.tab_stops(), stops);
        assert_eq!(p.access_key('c'), Some(Part::Shown(Shown::Classic)));
        assert_eq!(p.access_key('t'), Some(Part::Shown(Shown::Ticked)));
        assert_eq!(p.arrow(Part::Shown(Shown::Everything), true), Some(Part::Shown(Shown::Classic)));
        p.set_programs(program_rows(3, 2));
        // Rows: 0 Programs, 1 Games, 2 to 4 Game 00 to 02, 5 Notepad 01, 6 Paint 00. With
        // everything, every box is ticked, and the folder is closed.
        assert_eq!(p.program_rows(), [0, 1, 5, 6]);
        assert!((0..7).all(|i| p.row_tick(i) == Tick::On) && !p.changed);
        // The classic programs: Bevel95's own two, and their folder opens to show them.
        assert!(p.choose(Part::Shown(Shown::Classic)) && p.changed);
        assert_eq!(p.program_rows(), [0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(
            [0, 1, 2, 3, 4, 5].map(|i| p.row_tick(i)),
            [Tick::Mixed, Tick::Mixed, Tick::On, Tick::On, Tick::Off, Tick::Off]
        );
        // A box flipped now starts from the classic programs: Notepad joins them, and the
        // choice becomes "Only the programs ticked here".
        assert!(p.choose(Part::ProgramRow(5)) && p.programs.edited);
        assert_eq!(p.values.programs, Shown::Ticked);
        assert_eq!(ticked(&p), ["Games\\Game 00", "Games\\Game 01", "Notepad 01"]);
        assert!(!p.choose(Part::ProgramRow(7)), "no such row");
        // The same from everything: all but the program whose box was flipped.
        assert!(p.choose(Part::Shown(Shown::Everything)) && p.row_tick(4) == Tick::On);
        assert_eq!(ticked(&p).len(), 3, "the ticks themselves wait until a box is flipped");
        assert!(p.choose(Part::ProgramRow(6)));
        assert_eq!((p.values.programs, ticked(&p).len()), (Shown::Ticked, 4));
        // The choice is Bevel95's own: Windows' window leaves it as it is in effect.
        let now = Values { programs: Shown::Classic, ..values() };
        assert_eq!(Sheet::Taskbar.merge(p.values, now).programs, Shown::Classic);
        assert_eq!(Sheet::Bevel95.merge(p.values, now).programs, Shown::Ticked);
    }

    #[test]
    fn a_program_removed_elsewhere_loses_its_tick_here() {
        // Bevel95 Settings is open, showing everything, while Remove Shortcuts/Folders takes a
        // program out of the menu: the window shows the ticked choice, all but that program.
        let mut p = Properties::new(Sheet::Bevel95, Era::Win98, values());
        p.show_page(3);
        p.set_programs(program_rows(3, 2));
        p.programs_removed(&["Games\\Game 01".to_string()]);
        assert_eq!(p.values.programs, Shown::Ticked);
        assert_eq!(ticked(&p), ["Games\\Game 00", "Games\\Game 02", "Notepad 01", "Paint 00"]);
        assert!(!p.changed && !p.programs.edited, "it is in effect already: nothing for Apply to do");
        // A window whose list was never read only takes the choice.
        let mut p = Properties::new(Sheet::Taskbar, Era::Win98, values());
        p.programs_removed(&["Games\\Game 01".to_string()]);
        assert!(p.values.programs == Shown::Ticked && !p.programs.loaded);
    }

    #[test]
    fn a_program_added_elsewhere_gets_its_tick_here() {
        // Bevel95 Settings is open on the classic programs while Add...'s wizard makes a
        // shortcut, and the menu switched to the classic set plus the new program.
        let mut p = Properties::new(Sheet::Bevel95, Era::Win98, Values { programs: Shown::Classic, ..values() });
        p.show_page(3);
        p.set_programs(program_rows(3, 2));
        p.programs_added(&["Paint 00".to_string()], Shown::Ticked);
        assert_eq!(p.values.programs, Shown::Ticked);
        assert_eq!(ticked(&p), ["Games\\Game 00", "Games\\Game 01", "Paint 00"]);
        assert!(!p.changed && !p.programs.edited, "it is in effect already: nothing for Apply to do");
        // A menu that shows everything shows it anyway: the window stays as it is.
        let mut p = Properties::new(Sheet::Bevel95, Era::Win98, Values { programs: Shown::Classic, ..values() });
        p.show_page(3);
        p.set_programs(program_rows(3, 2));
        p.programs_added(&["Paint 00".to_string()], Shown::Everything);
        assert_eq!(p.values.programs, Shown::Classic);
        // A window whose list was never read only takes the choice.
        let mut p = Properties::new(Sheet::Taskbar, Era::Win98, values());
        p.programs_added(&["Paint 00".to_string()], Shown::Ticked);
        assert!(p.values.programs == Shown::Ticked && !p.programs.loaded);
    }

    #[test]
    fn the_list_of_programs_scrolls_opens_and_finds() {
        let mut p = Properties::new(Sheet::Bevel95, Era::Win95, Values { programs: Shown::Ticked, ..values() });
        p.show_page(3);
        p.set_programs(program_rows(3, 2));
        assert_eq!(programs_scroll(&p), None, "four rows show: no scroll bar");
        assert_eq!(program_rows_width(&p), 268);
        p.set_programs(program_rows(20, 30));
        // 52 rows; while the folder is closed 32 show (Programs, Games, 30 programs): a scroll
        // bar of 192, its track 160, the thumb 60 (160 × 12 / 32).
        let scroll = programs_scroll(&p).unwrap();
        assert_eq!((scroll.bar, scroll.total), (Rect::new(290, 149, 16, 192), 32));
        assert_eq!(scroll.thumb(0), Rect::new(290, 165, 16, 60));
        assert_eq!(scroll.thumb(20).bottom(), 325, "at the end the thumb reaches the lower button");
        assert_eq!(scroll.dragged(0, (160 - 60) / 2), 10, "dragged halfway: the middle");
        assert_eq!((scroll.dragged(0, -50), scroll.dragged(0, 500)), (0, 20));
        assert_eq!(program_rows_width(&p), 252);
        // Opened, the folder's 20 rows show too. The selection moves and the rows follow it.
        p.open_program(1, true);
        assert_eq!(programs_scroll(&p).unwrap().total, 52);
        p.move_program(11);
        assert_eq!((p.programs.sel, p.programs.top), (11, 0));
        p.move_program(1);
        assert_eq!((p.programs.sel, p.programs.top), (12, 1));
        p.move_program(100);
        assert_eq!((p.programs.sel, p.programs.top), (51, 40));
        // The folder closes: the list is shorter, the selection (not in it) stays.
        p.open_program(1, false);
        assert_eq!((p.programs.sel, p.programs.top), (51, 20));
        // A selection inside a folder that closes goes to the folder.
        p.open_program(1, true);
        p.programs.sel = 5;
        p.open_program(1, false);
        assert_eq!(p.programs.sel, 1);
        p.open_program(5, true);
        assert!(!p.programs.rows[5].open, "a program is no folder");
        p.move_program(-100);
        assert_eq!((p.programs.sel, p.programs.top), (0, 0));
        // Scrolling leaves the selection.
        p.scroll(100);
        assert_eq!((p.programs.sel, p.programs.top), (0, 20));
        // A letter goes to the next row of that letter that shows, around the end of the list.
        assert!(p.find_program('n') && p.programs.rows[p.programs.sel].name == "Notepad 01");
        assert_eq!(p.programs.top, 2, "scrolled back up so far that it shows, as the first row");
        assert!(p.find_program('N') && p.programs.rows[p.programs.sel].name == "Notepad 03");
        assert!(p.find_program('p') && p.programs.sel == 37, "the first Paint");
        p.move_program(100);
        assert!(p.find_program('p') && p.programs.sel == 0, "around the end: Programs itself");
        assert!(!p.find_program('z'));
        assert!(p.find_program('g') && p.programs.sel == 1, "the folder; its games do not show");
        // Right opens the folder, Right again goes in; Left goes back out, Left again closes.
        p.step_program(true);
        assert!(p.programs.rows[1].open && p.programs.sel == 1);
        p.step_program(true);
        assert_eq!(p.programs.sel, 2);
        p.step_program(true);
        assert_eq!(p.programs.sel, 2, "a program has nothing to open");
        p.step_program(false);
        assert_eq!(p.programs.sel, 1);
        p.step_program(false);
        assert!(!p.programs.rows[1].open && p.programs.sel == 1);
        p.step_program(false);
        assert_eq!(p.programs.sel, 0, "from a closed folder: to Programs");
        // "*" opens a folder with all that is in it.
        p.open_program(0, false);
        assert_eq!(p.program_rows(), [0]);
        p.open_all_programs();
        assert_eq!(p.program_rows().len(), 52);
        // Read again (after a change of look) with fewer programs: the ticks made in the
        // window stay with their programs, the folders stay open, the list keeps its place.
        assert!(p.choose(Part::ProgramRow(2)) && p.row_tick(2) == Tick::Off, "Game 00 unticked");
        p.move_program(100);
        p.refresh_programs(program_rows(3, 2));
        assert_eq!((p.programs.rows.len(), p.programs.sel, p.programs.top), (7, 0, 0));
        assert_eq!([2, 3].map(|i| p.row_tick(i)), [Tick::Off, Tick::On], "Game 00 is still unticked");
        assert!(p.programs.rows[1].open && p.programs.loaded && p.programs.edited);
    }

    #[test]
    fn the_list_of_programs_takes_clicks_and_keys() {
        use crate::kit::Form;
        let font = one_glyph_font();
        let fonts = Fonts { regular: &font, bold: &font };
        let plain = Mods::default();
        let mut w = PropertiesWindow::new(Properties::new(Sheet::Bevel95, Era::Win2000, values()), &fonts);
        w.props.show_page(3);
        w.props.set_programs(program_rows(20, 30));
        w.layout(&fonts);
        // The rows that show: Programs, Games (closed), then the 30 programs. Games stands one
        // step in: its plus box is under Programs' picture (4 to 12), its check box from 19, its
        // picture and name from 35.
        let games = w.rect(Part::ProgramRow(1)).unwrap();
        assert_eq!(games, Rect::new(38, 149 + 16, 252, 16));
        assert_eq!(w.hit(games.x + 40, games.y + 5), Some(Part::ProgramRow(1)));
        w.press(Some(Part::ProgramRow(1)), games.x + 8, games.y + 8);
        assert!(w.props.programs.rows[1].open && w.props.focus == Part::Programs);
        assert_eq!((w.props.program_rows().len(), w.props.programs.sel), (52, 0));
        w.press(Some(Part::ProgramRow(1)), games.x + 8, games.y + 8);
        assert_eq!(w.props.program_rows().len(), 32, "the minus box puts them away again");
        // A click on its name selects it; a double-click there opens it.
        w.press(Some(Part::ProgramRow(1)), games.x + 60, games.y + 8);
        assert_eq!(w.props.programs.sel, 1);
        assert_eq!(w.double_click(games.x + 60, games.y + 8), Some(vec![]));
        assert!(w.props.programs.rows[1].open);
        assert_eq!(w.double_click(games.x + 25, games.y + 8), None, "on its check box: one more press");
        assert_eq!(w.double_click(5, 5), None);
        w.layout(&fonts);
        // The scroll bar: a button, the track (a page), the wheel (three rows), the thumb.
        assert_eq!(w.press(Some(Part::ScrollDown), 0, 0), Press::Hold(Part::ScrollDown));
        assert_eq!(w.props.programs.top, 1);
        w.press(Some(Part::ScrollPageDown), 0, 0);
        assert_eq!(w.props.programs.top, 13);
        w.wheel(1);
        assert_eq!(w.props.programs.top, 10);
        assert_eq!(w.press(Some(Part::ScrollThumb), 0, 0), Press::Grab(Part::ScrollThumb, 10));
        assert!(w.drag(Part::ScrollThumb, 100, 10, 0, 40) && w.props.programs.top == 0);
        w.layout(&fonts);
        // Everything shows. Game 01 stands two steps in, its check box from 38: a click on it
        // takes its tick away, starting from everything, and the choice becomes the ticked
        // programs.
        let game = w.rect(Part::ProgramRow(3)).unwrap();
        assert_eq!(game.y, 149 + 3 * 16);
        w.press(Some(Part::ProgramRow(3)), game.x + 38 + 8, game.y + 8);
        assert_eq!(w.props.values.programs, Shown::Ticked);
        assert_eq!((w.props.row_tick(3), w.props.row_tick(1), w.props.changed), (Tick::Off, Tick::Mixed, true));
        assert_eq!(w.props.programs.sel, 3);
        // Keys in the list: Left goes to the folder; Space on it ticks all of it (it was half
        // ticked), Space again unticks all of it.
        w.key(Key::Left, plain);
        assert_eq!(w.props.programs.sel, 1);
        w.space();
        assert_eq!((w.props.row_tick(1), w.props.row_tick(3)), (Tick::On, Tick::On));
        w.space();
        assert_eq!((w.props.row_tick(1), w.props.row_tick(0)), (Tick::Off, Tick::Mixed));
        // "-" closes it, "+" opens it, Right goes in; "-" on a program does nothing.
        w.typed('-');
        assert!(!w.props.programs.rows[1].open);
        w.typed('+');
        w.key(Key::Right, plain);
        assert_eq!((w.props.programs.rows[1].open, w.props.programs.sel), (true, 2));
        w.typed('-');
        assert_eq!(w.props.program_rows().len(), 52);
        // A letter, End, Page Up, Home; "*" on Programs after "-" brings everything back.
        w.typed('n');
        assert_eq!(w.props.programs.rows[w.props.programs.sel].name, "Notepad 01");
        w.key(Key::End, plain);
        assert_eq!((w.props.programs.sel, w.props.programs.top), (51, 40));
        w.key(Key::PageUp, plain);
        assert_eq!(w.props.programs.sel, 40);
        w.key(Key::Home, plain);
        w.typed('-');
        assert_eq!(w.props.program_rows(), [0]);
        w.typed('*');
        assert_eq!(w.props.program_rows().len(), 52);
    }

    /// A font with one glyph (every character is drawn as it): enough to lay a window out.
    fn one_glyph_font() -> Font {
        let glyph = crate::text::Glyph { ch: '?', advance: 6, ink_x: 0, ink_w: 0, ink: Vec::new() };
        Font::new("test".into(), false, 13, 11, vec![glyph]).unwrap()
    }

    #[test]
    fn ctrl_tab_turns_each_windows_own_pages() {
        use crate::kit::Form;
        let font = one_glyph_font();
        let fonts = Fonts { regular: &font, bold: &font };
        let ctrl = Mods { shift: false, ctrl: true };
        let mut taskbar = PropertiesWindow::new(Properties::new(Sheet::Taskbar, Era::Win95, values()), &fonts);
        assert_eq!(taskbar.layout.tabs.len(), 2, "Windows' two pages, and no tab of ours");
        taskbar.key(Key::Tab, ctrl);
        assert_eq!((taskbar.props.page, taskbar.props.focus), (1, Part::Add));
        taskbar.key(Key::Tab, ctrl);
        assert_eq!(taskbar.props.page, 0, "and round again");
        let mut settings = PropertiesWindow::new(Properties::new(Sheet::Bevel95, Era::Win95, values()), &fonts);
        assert_eq!((settings.layout.title, settings.layout.tabs.len()), ("Bevel95 Settings", 4));
        settings.key(Key::Tab, ctrl);
        let focus = Part::WinKey(WindowsKey::Find);
        assert_eq!((settings.props.current_page(), settings.props.focus), (Page::OwnTaskbar, focus));
        // Laid out again (the window host does that after every event), the box takes clicks
        // on its label too.
        settings.layout(&fonts);
        let r = settings.rect(Part::Check(Check::Group)).unwrap();
        assert_eq!(settings.hit(r.x + 20, r.y + 5), Some(Part::Check(Check::Group)));
        settings.key(Key::Tab, ctrl);
        assert_eq!((settings.props.current_page(), settings.props.focus), (Page::Desktop, Part::Check(Check::Desktop)));
        settings.key(Key::Tab, Mods { shift: true, ctrl: true });
        settings.key(Key::Tab, Mods { shift: true, ctrl: true });
        assert_eq!(settings.props.current_page(), Page::Bevel95);
    }

    #[test]
    fn pictures_have_the_right_sizes() {
        let mut all: Vec<&Picture> =
            vec![&IE4_CUSTOMIZE_ICON, &IE4_DOCUMENTS_ICON, &W2K_CUSTOMIZE_ICON, &W2K_CLEAR_ICON];
        for set in [&W95_PREVIEW, &IE4_PREVIEW, &W2K_PREVIEW] {
            all.extend([&set.window, &set.start_large, &set.start_small, &set.taskbar, &set.taskbar_no_clock]);
        }
        for pic in all {
            assert_eq!(pic.rgb.len() as i32, pic.w * pic.h * 3);
        }
    }
}
